use crate::socket::{InitMsg, SctpSocket};
use crate::stream::SctpStream;
use crate::wrap_io_error;
use socket2::Domain;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::unix::AsyncFd;

/// A structure representing a socket server
///
/// # Examples
///
/// ```no_run
/// use tokio_sctp::SctpListener;
///
/// use std::io;
///
/// #[tokio::main]
/// async fn main() -> io::Result<()> {
///     let listener = SctpListener::bind("127.0.0.1:2345".parse().unwrap())?;
///
///     // use the listener
///
///     # let _ = listener;
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct SctpListener {
    inner: AsyncFd<SctpSocket>,
}

impl SctpListener {
    /// Create a listener bound to a single address
    ///
    /// Sets the `SO_REUSEADDR` option on the socket.
    pub fn bind(addr: SocketAddr) -> io::Result<SctpListener> {
        let socket = SctpSocket::new(Domain::for_address(addr))
            .map_err(|e| wrap_io_error("Failed to create socket", e))?;
        socket
            .set_reuseaddr(true)
            .map_err(|e| wrap_io_error("Failed to set SO_REUSEADDR", e))?;
        socket
            .bind_sys(addr)
            .map_err(|e| wrap_io_error("Failed to bind socket", e))?;
        socket
            .listen(-1)
            .map_err(|e| wrap_io_error("Failed to listen on socket", e))?;
        socket
            .set_nonblocking(true)
            .map_err(|e| wrap_io_error("Failed to set O_NONBLOCK", e))?;
        Ok(SctpListener {
            inner: AsyncFd::new(socket)?,
        })
    }

    /// Create a multi-homed listener
    ///
    /// Sets the `SO_REUSEADDR` option on the socket.
    pub fn bindx(addrs: &[SocketAddr]) -> io::Result<SctpListener> {
        let mut raw_addrs = addrs
            .iter()
            .map(|a| unsafe { *socket2::SockAddr::from(*a).as_ptr() })
            .collect::<Vec<_>>();
        let Some(addr) = addrs.first() else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Addresses empty"))
        };
        let socket = SctpSocket::new(Domain::for_address(*addr))?;
        socket
            .set_reuseaddr(true)
            .map_err(|e| wrap_io_error("Failed to set SO_REUSEADDR", e))?;
        socket
            .bindx_sys(&mut raw_addrs)
            .map_err(|e| wrap_io_error("Failed to bind socket", e))?;
        socket
            .listen(-1)
            .map_err(|e| wrap_io_error("Failed to listen on socket", e))?;
        socket
            .set_nonblocking(true)
            .map_err(|e| wrap_io_error("Failed to set O_NONBLOCK", e))?;
        Ok(SctpListener {
            inner: AsyncFd::new(socket)?,
        })
    }

    /// Create a listener from an already configured socket
    pub fn bind_from(socket: SctpSocket, addr: SocketAddr) -> io::Result<SctpListener> {
        socket
            .bind_sys(addr)
            .map_err(|e| wrap_io_error("Failed to bind socket", e))?;
        socket
            .listen(-1)
            .map_err(|e| wrap_io_error("Failed to listen on socket", e))?;
        socket
            .set_nonblocking(true)
            .map_err(|e| wrap_io_error("Failed to set O_NONBLOCK", e))?;
        Ok(SctpListener {
            inner: AsyncFd::new(socket)?,
        })
    }

    /// Create a listener from an already configured socket
    pub fn bindx_from(socket: SctpSocket, addrs: &[SocketAddr]) -> io::Result<SctpListener> {
        let mut addrs = addrs
            .iter()
            .map(|a| unsafe { *socket2::SockAddr::from(*a).as_ptr() })
            .collect::<Vec<_>>();

        socket
            .bindx_sys(&mut addrs)
            .map_err(|e| wrap_io_error("Failed to bind socket", e))?;
        socket
            .listen(-1)
            .map_err(|e| wrap_io_error("Failed to listen on socket", e))?;
        socket
            .set_nonblocking(true)
            .map_err(|e| wrap_io_error("Failed to set O_NONBLOCK", e))?;
        Ok(SctpListener {
            inner: AsyncFd::new(socket)?,
        })
    }

    /// Accept a new connection
    pub async fn accept(&self) -> io::Result<(SctpStream, SocketAddr)> {
        loop {
            let mut guard = self.inner.readable().await?;
            match guard.try_io(|inner| inner.get_ref().accept()) {
                Ok(Ok((socket, addr))) => {
                    socket.set_nonblocking(true)?;
                    return SctpStream::new(socket).map(|stream| (stream, addr));
                }
                Ok(Err(err)) => return Err(err),
                Err(_would_block) => {}
            };
        }
    }

    /// Returns the local socket address of this listener.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.get_ref().local_addr()
    }

    /// Returns the remote socket address of this listener.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.get_ref().peer_addr()
    }

    /// Set `timeout` in seconds on accept
    pub fn set_timeout(&self, timeout: Duration) -> io::Result<()> {
        self.inner.get_ref().set_read_timeout(Some(timeout))
    }

    /// Set the value for the `SCTP_INITMSG` option on this socket
    pub fn set_sctp_initmsg(&self, init: &InitMsg) -> io::Result<()> {
        self.inner.get_ref().set_sctp_initmsg(init)
    }

    /// Sets the value of the `SCTP_NODELAY` option on this socket.
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.inner.get_ref().set_nodelay(nodelay)
    }

    /// Gets the value of the `SCTP_NODELAY` option on this socket.
    pub fn nodelay(&self) -> io::Result<bool> {
        self.inner.get_ref().nodelay()
    }
}
