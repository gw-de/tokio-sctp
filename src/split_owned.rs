use std::io;
use std::net::SocketAddr;

use std::sync::Arc;

use bytes::BufMut;
use tokio::io::ReadBuf;

use crate::{RecvFlags, RecvInfo, SctpStream, SendOptions};

/// SCTP equivalent of tokio's TCP OwnedReadHalf
#[derive(Debug)]
pub struct OwnedReadHalf {
    inner: Arc<SctpStream>,
}

/// SCTP equivalent of tokio's TCP OwnedWriteHalf
#[derive(Debug)]
pub struct OwnedWriteHalf {
    inner: Arc<SctpStream>,
    shutdown_on_drop: bool,
}

pub(crate) fn split_owned(stream: SctpStream) -> (OwnedReadHalf, OwnedWriteHalf) {
    let arc = Arc::new(stream);
    let read = OwnedReadHalf {
        inner: Arc::clone(&arc),
    };
    let write = OwnedWriteHalf {
        inner: arc,
        shutdown_on_drop: true,
    };
    (read, write)
}

impl OwnedReadHalf {
    /// [`SctpStream::recvmsg`]: SctpStream::recvmsg
    pub async fn recvmsg<'a>(
        &'a mut self,
        buf: &'a mut ReadBuf<'a>,
    ) -> io::Result<(usize, RecvInfo, RecvFlags)> {
        self.inner.recvmsg(buf).await
    }

    /// [`SctpStream::recvmsg_buf`]: SctpStream::recvmsg
    pub async fn recvmsg_buf<'a, B: BufMut>(
        &'a mut self,
        buf: &'a mut B,
    ) -> io::Result<(usize, RecvInfo, RecvFlags)> {
        self.inner.recvmsg_buf(buf).await
    }

    /// [`SctpStream::recvmsg_eor_buf`]: SctpStream::recvmsg_eor_buf
    pub async fn recvmsg_eor_buf<'a, B: BufMut>(
        &'a mut self,
        buf: &'a mut B,
    ) -> io::Result<(usize, RecvInfo, RecvFlags)> {
        self.inner.recvmsg_eor_buf(buf).await
    }
}

impl OwnedWriteHalf {
    /// [`SctpStream::sendmsg`]: SctpStream::sendmsg
    pub async fn sendmsg<'a>(
        &'a mut self,
        buf: &'a [u8],
        to: Option<SocketAddr>,
        opts: &SendOptions,
    ) -> io::Result<usize> {
        self.inner.sendmsg(buf, to, opts).await
    }
}

impl Drop for OwnedWriteHalf {
    fn drop(&mut self) {
        if self.shutdown_on_drop {
            let _ = self.inner.shutdown(std::net::Shutdown::Write);
        }
    }
}
