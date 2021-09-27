#![allow(unused)]

use libc::{c_int, c_uint};

pub const SOL_SCTP: c_int = 132;
pub const SOCK_SEQPACKET: c_int = 5;

#[allow(non_camel_case_types)]
pub type sctp_assoc_t = c_uint;

/// The sctp_sndrcvinfo type
#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct SctpSndRcvInfo {
    /// Stream sending to
    pub stream: u16,
    /// Valid for recv only
    pub ssn: u16,
    /// Flags to control sending
    pub flags: u16,
    /// ppid field
    pub ppid: u32,
    /// context field
    pub context: u32,
    /// timetolive for PR-SCTP
    pub timetolive: u32,
    /// valid for recv only
    pub tsn: u32,
    /// valid for recv only
    pub cumtsn: u32,
    /// The association id
    pub assoc_id: sctp_assoc_t,
}
