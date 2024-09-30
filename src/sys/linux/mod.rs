#![allow(unused)]

pub mod consts;
pub use self::consts::*;

use libc::{c_int, c_ulong, c_ushort, c_void, size_t, sockaddr, socklen_t, ssize_t};

use super::common::{sctp_assoc_t, SctpSndRcvInfo};

extern "system" {
    pub fn sctp_bindx(sock: c_int, sock_addr: *mut sockaddr, num: c_int, ty: c_int) -> c_int;
    pub fn sctp_connectx(
        sock: c_int,
        sock_addr: *mut sockaddr,
        addrcnt: c_int,
        assoc: *mut sctp_assoc_t,
    ) -> c_int;
    pub fn sctp_freepaddrs(addrs: *mut sockaddr);
    pub fn sctp_freeladdrs(addrs: *mut sockaddr);
    pub fn sctp_getaddrlen(family: c_int) -> c_int;
    pub fn sctp_getpaddrs(s: c_int, assoc: sctp_assoc_t, addrs: *mut *mut sockaddr) -> c_int;
    pub fn sctp_getladdrs(s: c_int, assoc: sctp_assoc_t, addrs: *mut *mut sockaddr) -> c_int;
    pub fn sctp_opt_info(
        s: c_int,
        assoc: sctp_assoc_t,
        opt: c_int,
        arg: *mut c_void,
        size: *mut socklen_t,
    ) -> c_int;
    pub fn sctp_peeloff(s: c_int, assoc: sctp_assoc_t) -> c_int;
    // XXX: lksctp returns int not ssize_t
    // See https://github.com/sctp/lksctp-tools/issues/48
    pub fn sctp_recvmsg(
        s: c_int,
        msg: *mut c_void,
        len: size_t,
        from: *mut sockaddr,
        fromlen: *mut socklen_t,
        sinfo: *mut SctpSndRcvInfo,
        flags: *mut c_int,
    ) -> c_int;
    pub fn sctp_send(
        s: c_int,
        msg: *const c_void,
        len: size_t,
        sinfo: *const SctpSndRcvInfo,
        flags: c_int,
    ) -> ssize_t;
    // XXX: lksctp returns int not ssize_t
    // See https://github.com/sctp/lksctp-tools/issues/48
    pub fn sctp_sendmsg(
        s: c_int,
        msg: *const c_void,
        len: size_t,
        to: *mut sockaddr,
        tolen: socklen_t,
        ppid: c_ulong,
        flags: c_ulong,
        stream_no: c_ushort,
        ttl: c_ulong,
        ctx: c_ulong,
    ) -> c_int;
}
