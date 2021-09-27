//! # tokio-sctp
//!
//! `tokio-sctp` provides non-blocking SCTP socket bindings over the tokio runtime. This crate is
//! currently Linux only and only supports one-to-one style sockets. Building thus requires the
//! [lksctp-tools](https://github.com/sctp/lksctp-tools) package is installed on the system.
///
mod listener;
pub use listener::SctpListener;

mod socket;
pub use socket::*;

mod stream;
pub use stream::*;

mod split_owned;
pub use split_owned::*;

mod sys;

use std::io;

fn wrap_io_error(desc: &'static str, e: io::Error) -> io::Error {
    io::Error::new(e.kind(), WrappedIoErr(desc, e))
}

struct WrappedIoErr(&'static str, io::Error);

impl std::fmt::Display for WrappedIoErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::fmt::Debug for WrappedIoErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {:?}", self.0, self.1)
    }
}

impl std::error::Error for WrappedIoErr {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.1)
    }
}
