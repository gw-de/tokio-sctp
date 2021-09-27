use crate::socket::{RecvFlags, RecvInfo, SctpSocket};
use crate::{split_owned, wrap_io_error, OwnedReadHalf, OwnedWriteHalf, SendOptions, Status};
use bytes::BufMut;
use socket2::Domain;
use std::io;
use std::net::{Shutdown, SocketAddr};
use std::os::unix::io::{AsRawFd, RawFd};
use tokio::io::unix::AsyncFd;
use tokio::io::ReadBuf;

/// A connection to a SCTP endpoint
///
/// # Examples
///
/// ```no_run
/// use tokio_sctp::{SctpStream, SendOptions};
/// use std::error::Error;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn Error>> {
///     // Connect to a peer
///     let mut stream = SctpStream::connect("127.0.0.1:8080".parse().unwrap()).await?;
///
///     // Write some data.
///     stream.sendmsg(b"hello world!", None, &SendOptions::default()).await?;
///
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct SctpStream {
    pub(crate) inner: AsyncFd<SctpSocket>,
}

impl SctpStream {
    pub fn new(socket: SctpSocket) -> io::Result<Self> {
        Ok(SctpStream {
            inner: AsyncFd::new(socket)?,
        })
    }

    /// Create a new SCTP stream and issue a non-blocking connect to the specified address.
    pub async fn connect(addr: SocketAddr) -> io::Result<Self> {
        let socket = SctpSocket::new(Domain::for_address(addr))
            .map_err(|e| wrap_io_error("Failed to create socket", e))?;
        socket
            .set_nonblocking(true)
            .map_err(|e| wrap_io_error("Failed to set socket non-blocking", e))?;

        SctpStream::connect_inner(socket, |s| s.connect_sys(addr)).await
    }

    /// Create a new SCTP stream and issue a non-blocking connect to the specified addresses.
    pub async fn connectx(addrs: &[SocketAddr]) -> io::Result<Self> {
        let Some(addr) = addrs.first() else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Addresses empty"))
        };
        let socket = SctpSocket::new(Domain::for_address(*addr))
            .map_err(|e| wrap_io_error("Failed to create socket", e))?;
        socket
            .set_nonblocking(true)
            .map_err(|e| wrap_io_error("Failed to set socket non-blocking", e))?;

        let mut addrs = addrs
            .iter()
            .map(|a| unsafe { *socket2::SockAddr::from(*a).as_ptr() })
            .collect::<Vec<_>>();
        SctpStream::connect_inner(socket, |s| s.connectx_sys(&mut addrs)).await
    }

    /// Creates a stream from an already configured socket
    pub async fn connect_from(socket: SctpSocket, addr: SocketAddr) -> io::Result<Self> {
        socket
            .set_nonblocking(true)
            .map_err(|e| wrap_io_error("Failed to set socket non-blocking", e))?;

        SctpStream::connect_inner(socket, |s| s.connect_sys(addr)).await
    }

    /// Creates a stream from an already configured socket to multi-homed endpoint
    pub async fn connectx_from(socket: SctpSocket, addrs: &[SocketAddr]) -> io::Result<Self> {
        socket
            .set_nonblocking(true)
            .map_err(|e| wrap_io_error("Failed to set socket non-blocking", e))?;

        let mut addrs = addrs
            .iter()
            .map(|a| unsafe { *socket2::SockAddr::from(*a).as_ptr() })
            .collect::<Vec<_>>();
        SctpStream::connect_inner(socket, |s| s.connectx_sys(&mut addrs)).await
    }

    async fn connect_inner<F>(socket: SctpSocket, mut sys_call: F) -> io::Result<Self>
    where
        F: FnMut(&SctpSocket) -> io::Result<()>,
    {
        match sys_call(&socket) {
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) if err.raw_os_error() == Some(libc::EINPROGRESS) => {}
            Err(err) => return Err(wrap_io_error("Failed to connect", err)),
            Ok(_) => {}
        };

        let stream =
            SctpStream::new(socket).map_err(|e| wrap_io_error("Failed to create SctpStream", e))?;

        // Once we've connected, wait for the stream to be writable as
        // that's when the actual connection has been initiated.
        let _ = stream
            .inner
            .writable()
            .await
            .map_err(|e| wrap_io_error("Failed to wait until writable", e))?;
        // Check if we hit an error on connect
        if let Some(e) = stream.inner.get_ref().take_error()? {
            return Err(wrap_io_error("Connect error", e));
        }
        Ok(stream)
    }

    /// Shuts down the read, write, or both halves of this connection.
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.get_ref().shutdown(how)
    }

    /// Returns the socket address of the local half of this SCTP connection.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.get_ref().local_addr()
    }

    /// Returns the socket address of the remote peer of this SCTP connection.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.get_ref().peer_addr()
    }

    /// Sets the value of the `SCTP_NODELAY` option on this socket.
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.inner.get_ref().set_nodelay(nodelay)
    }

    /// Gets the value of the `SCTP_NODELAY` option on this socket.
    pub fn nodelay(&self) -> io::Result<bool> {
        self.inner.get_ref().nodelay()
    }

    /// Get the value of the `SCTP_STATUS` option on this socket.
    pub fn status(&self) -> io::Result<Status> {
        self.inner.get_ref().status()
    }

    /// Send a message on a given stream
    pub async fn sendmsg<'a>(
        &'a self,
        buf: &'a [u8],
        to: Option<SocketAddr>,
        opts: &SendOptions,
    ) -> io::Result<usize> {
        loop {
            let mut guard = self.inner.writable().await?;
            match guard.try_io(|inner| inner.get_ref().sendmsg(buf, to, opts)) {
                Ok(res) => return res,
                Err(_would_block) => {}
            };
        }
    }

    /// Receive a message from a socket on a given stream.
    pub async fn recvmsg<'a>(
        &'a self,
        buf: &'a mut ReadBuf<'a>,
    ) -> io::Result<(usize, RecvInfo, RecvFlags)> {
        loop {
            let mut guard = self.inner.readable().await?;
            match guard.try_io(|inner| inner.get_ref().recvmsg(buf)) {
                Ok(Ok(res)) => return Ok(res),
                Ok(err) => return err,
                Err(_would_block) => {}
            };
        }
    }

    /// Receive a message from a socket on a given stream into a BufMut.
    pub async fn recvmsg_buf<'a, B: BufMut>(
        &'a self,
        buf: &'a mut B,
    ) -> io::Result<(usize, RecvInfo, RecvFlags)> {
        // SAFETY: we're only reading initialized memory into dst as guarenteed by recvmsg
        let dst = unsafe { buf.chunk_mut().as_uninit_slice_mut() };
        let mut dst = ReadBuf::uninit(dst);

        match self.recvmsg(&mut dst).await {
            Ok(res) => {
                unsafe { buf.advance_mut(res.0) };
                Ok(res)
            }
            Err(err) => Err(err),
        }
    }

    /// Receive a full message into the buffer.
    ///
    /// NOTE: This implementation depends on the BufMut implementation to grow the buffer on call
    /// to chunk_mut
    ///
    /// Returns the last ReceiveInfo and MsgFlags.
    ///
    /// # Cancel safety
    ///
    /// This method is not cancel safe, as it makes multiple calls to the underlying syscall
    pub async fn recvmsg_eor_buf<'a, B: BufMut>(
        &'a self,
        buf: &'a mut B,
    ) -> io::Result<(usize, RecvInfo, RecvFlags)> {
        let mut total = 0;
        loop {
            let (n, rcvinfo, flags) = self.recvmsg_buf(buf).await?;
            if n == 0 {
                return Ok((n, rcvinfo, flags));
            }
            total += n;
            if flags.contains(RecvFlags::EOR) {
                return Ok((total, rcvinfo, flags));
            }
        }
    }

    /// Splits a `SctpStream` into a read half and a write half, which can be used
    /// to read and write the stream concurrently.
    ///
    /// **Note:** Dropping the write half will shut down the write half of the SCTP
    /// stream. This is equivalent to calling `shutdown()` on the `SctpStream`.
    pub fn into_split(self) -> (OwnedReadHalf, OwnedWriteHalf) {
        split_owned(self)
    }
}

impl AsRawFd for SctpStream {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.inner.get_ref().as_raw_fd()
    }
}
