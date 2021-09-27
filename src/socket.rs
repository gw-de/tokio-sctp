use bitflags::bitflags;
use libc::IPPROTO_SCTP;
use socket2::{Domain, SockAddr, Socket, Type};
use std::convert::From;
use std::io::{self, Read, Write};
use std::mem::MaybeUninit;
use std::net::{Shutdown, SocketAddr};
use std::os::unix::io::AsRawFd;
use std::time::Duration;
use tokio::io::ReadBuf;

use crate::sys::common::{sctp_assoc_t, SctpSndRcvInfo};
use crate::{SctpListener, SctpStream};

use crate::sys::linux::*;

/// `SctpSocket` wraps an operating system socket and enables the caller to
/// configure the socket before establishing a SCTP assocation or accepting
/// inbound connections. The caller is able to set socket option and explicitly
/// bind the socket with a socket address.
///
/// The underlying socket is closed when the `SctpSocket` value is dropped.
#[derive(Debug)]
pub struct SctpSocket {
    pub inner: Socket,
}

impl SctpSocket {
    /// Create a new SCTP socket configured for the given domain.
    pub fn new(domain: Domain) -> io::Result<Self> {
        let s = Socket::new(domain, Type::STREAM, Some(libc::IPPROTO_SCTP.into()))
            .and_then(|s| SctpSocket::enable_notifications(&s).map(|_| s))
            .map(|socket| SctpSocket { inner: socket })?;
        Ok(s)
    }

    // Enables sctp_sndrcvinfo structure to be filled in on sctp_recvmsg(3)
    fn enable_notifications(socket: &Socket) -> io::Result<()> {
        let sub: EventSubscribe = EventSubscribe {
            data_io_event: 1,
            ..Default::default()
        };

        unsafe {
            match libc::setsockopt(
                socket.as_raw_fd(),
                IPPROTO_SCTP,
                SCTP_EVENTS,
                &sub as *const EventSubscribe as *const libc::c_void,
                std::mem::size_of::<EventSubscribe>() as u32,
            ) {
                r if r >= 0 => Ok(()),
                _ => Err(io::Error::last_os_error()),
            }
        }
    }

    /// Initiate a connection on this socket to the specified address.
    ///
    /// NOTE: The caller must ensure that the given socket is non-blocking
    pub async fn connect(self, addr: SocketAddr) -> io::Result<SctpStream> {
        SctpStream::connect_from(self, addr).await
    }

    /// Initiate a connection to a multi-homed endpoint.
    ///
    /// NOTE: The caller must ensure that the given socket is non-blocking
    pub async fn connectx(self, addrs: &[SocketAddr]) -> io::Result<SctpStream> {
        SctpStream::connectx_from(self, addrs).await
    }

    /// Initiate a connection on this socket to the specified address.
    ///
    /// This directly corresponds to `connect(2)`. To connect async into a `SctpStream` see
    /// `connect`
    ///
    /// An error will be returned if `listen` or `connect` has already been
    /// called on this builder.
    ///
    /// # Notes
    ///
    /// When using a non-blocking connect (by setting the socket into
    /// non-blocking mode before calling this function), socket option can't be
    /// set *while connecting*. Socket options can be safely set before and
    /// after connecting the socket.
    pub fn connect_sys(&self, addr: SocketAddr) -> io::Result<()> {
        self.inner.connect(&SockAddr::from(addr))
    }

    /// Initiate a connection to a multi-homed endpoint.
    ///
    /// This directly corresponds to `sctp_connectx(3)`. To connectx async into a `SctpStream` see
    /// `connectx`
    ///
    /// The way the SCTP stack uses the list of addresses to set up the association is implementation dependent.
    /// This function only specifies that the stack will try to make use of all of the addresses in the list when needed.
    /// Note that the list of addresses passed in is only used for setting up
    /// the association.  It does not necessarily equal the set of addresses
    /// the peer uses for the resulting association.  If the caller wants to
    /// find out the set of peer addresses, it must use sctp_getpaddrs() to
    /// retrieve them after the association has been set up.
    ///
    pub fn connectx_sys(&self, addrs: &mut [libc::sockaddr]) -> io::Result<()> {
        unsafe {
            match sctp_connectx(
                self.as_raw_fd(),
                addrs.as_mut_ptr(),
                addrs.len().try_into().unwrap(),
                std::ptr::null_mut(),
            ) {
                -1 => Err(io::Error::last_os_error()),
                _ => Ok(()),
            }
        }
    }

    /// Binds this socket to the specified address.
    pub async fn bind(self, addr: SocketAddr) -> io::Result<SctpListener> {
        SctpListener::bind_from(self, addr)
    }

    /// Binds this socket to the specified addresses to set up a multi-homed endpoint.
    pub async fn bindx(self, addrs: &[SocketAddr]) -> io::Result<SctpListener> {
        SctpListener::bindx_from(self, addrs)
    }

    /// Binds this socket to the specified address.
    ///
    /// This directly corresponds to `bind(2)`. To bind async into a `SctpStream` see
    /// `bind`
    pub fn bind_sys(&self, addr: SocketAddr) -> io::Result<()> {
        self.inner.bind(&socket2::SockAddr::from(addr))
    }

    /// This function allows the user to bind a specific subset of addresses
    ///
    /// This directly corresponds to `sctp_bindx(3)`. To bindx async into a `SctpStream` see
    /// `bindx`
    pub fn bindx_sys(&self, addrs: &mut [libc::sockaddr]) -> io::Result<()> {
        unsafe {
            match sctp_bindx(
                self.as_raw_fd(),
                addrs.as_mut_ptr(),
                addrs.len().try_into().unwrap(),
                SCTP_BINDX_ADD_ADDR,
            ) {
                -1 => Err(io::Error::last_os_error()),
                _ => Ok(()),
            }
        }
    }

    /// Mark a socket as ready to accept incoming connection requests using
    /// [`Socket::accept()`].
    ///
    /// An error will be returned if `listen` or `connect` has already been
    /// called on this builder.
    pub fn listen(&self, backlog: i32) -> io::Result<()> {
        self.inner.listen(backlog)
    }

    /// Accept a new incoming connection from this listener.
    pub fn accept(&self) -> io::Result<(SctpSocket, SocketAddr)> {
        let (socket, addr) = self.inner.accept()?;

        let addr = addr
            .as_socket()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Address is not valid"))?;

        let stream = SctpSocket { inner: socket };

        Ok((stream, addr))
    }

    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O on the specified
    /// portions to return immediately with an appropriate value.
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.shutdown(how)
    }

    /// Returns the socket address of the local half of this socket.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr().and_then(|addr| {
            addr.as_socket()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Address is not valid"))
        })
    }

    /// Returns the socket address of the remote peer of this socket.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.peer_addr().and_then(|addr| {
            addr.as_socket()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Address is not valid"))
        })
    }

    /// Send a message from a socket on a given stream
    pub fn sendmsg(
        &self,
        msg: &[u8],
        to: Option<SocketAddr>,
        opts: &SendOptions,
    ) -> io::Result<usize> {
        let len = msg.len() as libc::size_t;
        let to = to.map(socket2::SockAddr::from);

        unsafe {
            // XXX: lksctp returns int not ssize_t
            let res = sctp_sendmsg(
                self.as_raw_fd(),
                msg.as_ptr() as *const libc::c_void,
                len,
                to.as_ref()
                    .map(|t| t.as_ptr() as *mut _)
                    .unwrap_or(std::ptr::null_mut()),
                to.map(|t| t.len()).unwrap_or(0),
                opts.ppid.into(),
                opts.flags.into(),
                opts.stream,
                opts.ttl.into(),
                0,
            );
            match res {
                res if res > 0 => {
                    // sctp_sendmsg is atomic
                    debug_assert!(res as usize == msg.len());

                    Ok(res as usize)
                }
                _ => Err(io::Error::last_os_error()),
            }
        }
    }

    /// Receive a message from a socket
    pub fn recvmsg(&self, buf: &mut ReadBuf<'_>) -> io::Result<(usize, RecvInfo, RecvFlags)> {
        let mut flags: libc::c_int = 0;
        let mut info = SctpSndRcvInfo::default();

        unsafe {
            let unfilled: &mut [MaybeUninit<u8>] = buf.unfilled_mut();
            let len = unfilled.len() as libc::size_t;

            let res = sctp_recvmsg(
                self.as_raw_fd(),
                unfilled.as_mut_ptr() as *mut libc::c_void,
                len,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                (&mut info) as *mut SctpSndRcvInfo,
                &mut flags,
            );

            match res {
                n if n >= 0 => {
                    let n = n as usize;

                    // Safety: assume the OS will initialize what it reads into the buffer
                    buf.assume_init(n);
                    buf.advance(n);

                    let f = RecvFlags::from_bits_unchecked(flags);
                    Ok((n, info.into(), f))
                }
                _ => Err(io::Error::last_os_error()),
            }
        }
    }

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    /// Receives data on the socket from the remote adress to which it is
    /// connected, without removing that data from the queue. On success,
    /// returns the number of bytes peeked.
    ///
    /// Successive calls return the same data. This is accomplished by passing
    /// `MSG_PEEK` as a flag to the underlying `recv` system call.
    ///
    /// # Safety
    ///
    /// `peek` makes the same safety guarantees regarding the `buf`fer as
    /// [`recv`].
    ///
    /// [`recv`]: Socket::recv
    pub fn peek(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
        self.inner.peek(buf)
    }

    /// Sets the value of the SCTP_NODELAY option on this socket.
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self.as_raw_fd(),
                IPPROTO_SCTP,
                SCTP_NODELAY,
                nodelay as libc::c_int,
            )
        }
    }

    /// Gets the value of the SCTP_NODELAY option on this socket.
    pub fn nodelay(&self) -> io::Result<bool> {
        unsafe {
            getsockopt::<libc::c_int>(self.as_raw_fd(), IPPROTO_SCTP, SCTP_NODELAY).map(|r| r != 0)
        }
    }

    /// Set the value for the `O_NONBLOCK` option on this socket
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }

    /// Sets the value for the IP_TTL option on this socket.
    /// This value sets the time-to-live field that is used in every packet sent from this socket.
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.inner.set_ttl(ttl)
    }

    /// Get the value of the `IP_TTL` option for this socket.
    pub fn ttl(&self) -> io::Result<u32> {
        self.inner.ttl()
    }

    /// Set value for the `SO_RCVTIMEO` option on this socket.
    ///
    /// If `timeout` is `None`, then `read` and `recv` calls will block indefinitely.
    pub fn set_read_timeout(&self, duration: Option<Duration>) -> io::Result<()> {
        self.inner.set_read_timeout(duration)
    }

    /// Get value for the `SO_RCVTIMEO` option on this socket.
    ///
    /// If the returned timeout is `None`, then `read` and `recv` calls will block indefinitely.
    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.inner.read_timeout()
    }

    /// Set value for the `SO_LINGER` option on this socket.
    ///
    /// If `linger` is not `None`, a close(2) or shutdown(2) will not return
    /// until all queued messages for the socket have been successfully sent or
    /// the linger timeout has been reached. Otherwise, the call returns
    /// immediately and the closing is done in the background. When the socket
    /// is closed as part of exit(2), it always lingers in the background.
    pub fn set_linger(&self, duration: Option<Duration>) -> io::Result<()> {
        self.inner.set_linger(duration)
    }

    /// Get the value of the `SO_LINGER` option on this socket.
    pub fn linger(&self) -> io::Result<Option<Duration>> {
        self.inner.linger()
    }

    /// Set the value for the `SCTP_INITMSG` option on this socket
    pub fn set_sctp_initmsg(&self, init_msg: &InitMsg) -> io::Result<()> {
        unsafe {
            match libc::setsockopt(
                self.as_raw_fd(),
                IPPROTO_SCTP,
                SCTP_INITMSG,
                init_msg as *const InitMsg as *const libc::c_void,
                std::mem::size_of::<InitMsg>() as u32,
            ) {
                r if r >= 0 => Ok(()),
                _ => Err(io::Error::last_os_error()),
            }
        }
    }

    /// Get the value of the `SCTP_STATUS` option on this socket.
    pub fn status(&self) -> io::Result<Status> {
        unsafe { getsockopt::<Status>(self.as_raw_fd(), IPPROTO_SCTP, SCTP_STATUS) }
    }

    /// Get the value of the SO_REUSEADDR option on this socket.
    pub fn reuseaddr(&self) -> io::Result<bool> {
        self.inner.reuse_address()
    }

    /// Sets the value of the SO_REUSEADDR option on this socket.
    pub fn set_reuseaddr(&self, reuse: bool) -> io::Result<()> {
        self.inner.set_reuse_address(reuse)
    }
}

impl Read for SctpSocket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Read for &SctpSocket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&self.inner).read(buf)
    }
}

impl Write for SctpSocket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl Write for &SctpSocket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&self.inner).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&self.inner).flush()
    }
}

impl AsRawFd for SctpSocket {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.inner.as_raw_fd()
    }
}

/// Caller must ensure `T` is the correct type for `opt` and `val`.
pub(crate) unsafe fn setsockopt<T>(
    fd: libc::c_int,
    opt: libc::c_int,
    val: libc::c_int,
    payload: T,
) -> io::Result<()> {
    let payload = &payload as *const T as *const libc::c_void;
    match libc::setsockopt(
        fd,
        opt,
        val,
        payload,
        std::mem::size_of::<T>() as libc::socklen_t,
    ) {
        -1 => Err(io::Error::last_os_error()),
        _ => Ok(()),
    }
}

/// Caller must ensure `T` is the correct type for `opt` and `val`.
pub(crate) unsafe fn getsockopt<T>(
    fd: libc::c_int,
    opt: libc::c_int,
    val: libc::c_int,
) -> io::Result<T> {
    let mut payload: MaybeUninit<T> = MaybeUninit::uninit();
    let mut len = std::mem::size_of::<T>() as libc::socklen_t;
    match libc::getsockopt(fd, opt, val, payload.as_mut_ptr().cast(), &mut len) {
        -1 => Err(io::Error::last_os_error()),
        _ => Ok(payload.assume_init()),
    }
}

/// SCTP Initiation Structure
///
/// This cmsghdr structure provides information for initializing new SCTP
/// associations with sendmsg().  The SCTP_INITMSG socket option uses
/// this same data structure.  This structure is not used for recvmsg().
///
/// <https://datatracker.ietf.org/doc/html/rfc6458#section-5.3.1>
#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct InitMsg {
    /// This is an integer representing the number of
    /// streams to which the application wishes to be able to send.  This
    /// number is confirmed in the SCTP_COMM_UP notification and must be
    /// verified, since it is a negotiated number with the remote
    /// endpoint.  The default value of 0 indicates the use of the
    /// endpoint's default value.
    pub num_ostreams: u16,

    /// This value represents the maximum number of
    /// inbound streams the application is prepared to support.  This
    /// value is bounded by the actual implementation.  In other words,
    /// the user may be able to support more streams than the operating
    /// system.  In such a case, the operating-system limit overrides the
    /// value requested by the user.  The default value of 0 indicates the
    /// use of the endpoint's default value.
    pub max_instreams: u16,

    /// This integer specifies how many attempts the
    /// SCTP endpoint should make at resending the INIT.  This value
    /// overrides the system SCTP 'Max.Init.Retransmits' value.  The
    /// default value of 0 indicates the use of the endpoint's default
    /// value.  This is normally set to the system's default
    /// 'Max.Init.Retransmit' value.
    pub max_attempts: u16,

    /// This value represents the largest timeout or
    /// retransmission timeout (RTO) value (in milliseconds) to use in
    /// attempting an INIT.  Normally, the 'RTO.Max' is used to limit the
    /// doubling of the RTO upon timeout.  For the INIT message, this
    /// value may override 'RTO.Max'.  This value must not influence
    /// 'RTO.Max' during data transmission and is only used to bound the
    /// initial setup time.  A default value of 0 indicates the use of the
    /// endpoint's default value.  This is normally set to the system's
    /// 'RTO.Max' value (60 seconds).
    pub max_init_timeout: u16,
}

/// Peer Address Information
///
/// Applications can retrieve information about a specific peer address
/// of an association, including its reachability state, congestion
/// window, and retransmission timer values.  This information is
/// read-only.
#[derive(Clone, Debug)]
#[repr(C)]
pub struct PeerAddrInfo {
    /// This parameter is ignored for one-to-one style sockets.
    /// For one-to-many style sockets, this field may be filled by the
    /// application, and if so, this field will have priority in looking
    /// up the association instead of using the address specified in
    /// spinfo_address.  Note that if the address does not belong to the
    /// association specified, then this call will fail.  If the
    /// application does not fill in the spinfo_assoc_id, then the address
    /// will be used to look up the association, and on return, this field
    /// will have the valid association identifier.  In other words, this
    /// call can be used to translate an address into an association
    /// identifier.  Note that the predefined constants are not allowed
    /// for this option.
    pub assoc_id: sctp_assoc_t,

    /// This is filled by the application and contains the peer address of interest.
    pub address: libc::sockaddr_storage,

    /// This contains the peer address's state:
    ///    SCTP_UNCONFIRMED:  This is the initial state of a peer address.
    ///    SCTP_ACTIVE:  This state is entered the first time after path
    ///       verification.  It can also be entered if the state is
    ///       SCTP_INACTIVE and the path supervision detects that the peer
    ///       address is reachable again.
    ///    SCTP_INACTIVE:  This state is entered whenever a path failure is
    ///       detected.
    pub state: i32,

    /// This contains the peer address's current congestion window.
    pub cwnd: u32,

    /// This contains the peer address's current smoothed round-trip time calculation in milliseconds.
    pub srtt: u32,

    /// This contains the peer address's current retransmission timeout value in milliseconds.
    pub rto: u32,

    /// This is the current Path MTU of the peer address.  It is the number of bytes available in an SCTP packet for chunks.
    pub mtu: u32,
}

/// Receive specific subset of fields in `sctp_sndrcvinfo` returned on readmsg
#[derive(Debug)]
pub struct RecvInfo {
    /// The SCTP stack places the message's stream number in this value.
    pub stream: u16,

    /// This value contains the stream sequence
    /// number that the remote endpoint placed in the DATA chunk.  For
    /// fragmented messages, this is the same number for all deliveries of
    /// the message (if more than one recvmsg() is needed to read the
    /// message).
    pub ssn: u16,

    /// This value is the same information that was passed by the upper layer
    /// in the peer application.  Please note that the SCTP stack performs
    /// no byte order modification of this field.  For example, if the
    /// DATA chunk has to contain a given value in network byte order, the
    /// SCTP user has to perform the htonl() computation.
    pub ppid: u32,

    /// This field holds a Transmission Sequence Number (TSN) that was assigned to one of the SCTP DATA chunks.
    pub tsn: u32,

    /// This field will hold the current cumulative TSN as known by the underlying SCTP layer.
    pub cummulative_tsn: u32,

    /// This field may contain any of the following flags and is composed of a bitwise OR of these values.
    ///  recvmsg() flags:
    ///     SCTP_UNORDERED:  This flag is present when the message was sent unordered.
    pub flags: u16,
}

impl From<SctpSndRcvInfo> for RecvInfo {
    fn from(v: SctpSndRcvInfo) -> Self {
        RecvInfo {
            stream: v.stream,
            ssn: v.ssn,
            ppid: v.ppid,
            tsn: v.tsn,
            cummulative_tsn: v.cumtsn,
            flags: v.flags,
        }
    }
}

/// Parameters for sctp_sendmsg(3)
#[derive(Debug, Default)]
pub struct SendOptions {
    /// an opaque unsigned value that is passed to the remote end along with the message
    pub ppid: u32,
    /// flags parameter is composed of a bitwise OR of the following values...
    pub flags: u32,
    /// identifies the stream number that the application wishes to send this message to
    pub stream: u16,
    /// specifies the time duration in milliseconds. The sending side will expire the message if the message has not been sent to the peer within this time period. A value of 0 indicates that no timeout should occur on this message
    pub ttl: u32,
}

bitflags! {
    /// Flags returned from recvmsg()
    pub struct RecvFlags: i32 {
        const EOR = libc::MSG_EOR;
        /// <https://github.com/torvalds/linux/blob/e2b542100719a93f8cdf6d90185410d38a57a4c1/include/uapi/linux/sctp.h#L181>
        const NOTIFICATION = 0x8000;
    }
}

/// Argument for the SCTP_EVENTS socket option
///
/// <https://datatracker.ietf.org/doc/html/rfc6458#section-6.2.1>
#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
struct EventSubscribe {
    /// Setting this flag to 1 will cause the reception
    /// of SCTP_SNDRCV information on a per-message basis.  The
    /// application will need to use the recvmsg() interface so that it
    /// can receive the event information contained in the msg_control
    /// field.  Setting the flag to 0 will disable the reception of the
    /// message control information.  Note that this flag is not really a
    /// notification and is stored in the ancillary data (msg_control),
    /// not in the data part (msg_iov).
    data_io_event: u8,

    /// Setting this flag to 1 will enable the
    /// reception of association event notifications.  Setting the flag to
    /// 0 will disable association event notifications.
    association_event: u8,

    /// Setting this flag to 1 will enable the reception
    /// of address event notifications.  Setting the flag to 0 will
    /// disable address event notifications.
    address_event: u8,

    /// Setting this flag to 1 will enable the
    /// reception of send failure event notifications.  Setting the flag
    /// to 0 will disable send failure event notifications.
    send_failure_event: u8,

    /// Setting this flag to 1 will enable the
    /// reception of peer error event notifications.  Setting the flag to
    /// 0 will disable peer error event notifications.
    peer_error_event: u8,

    /// Setting this flag to 1 will enable the
    /// reception of shutdown event notifications.  Setting the flag to 0
    /// will disable shutdown event notifications.
    shutdown_event: u8,

    /// Setting this flag to 1 will enable the
    /// reception of partial delivery event notifications.  Setting the
    /// flag to 0 will disable partial delivery event notifications.
    partial_delivery_event: u8,

    /// Setting this flag to 1 will enable the
    /// reception of adaptation layer event notifications.  Setting the
    /// flag to 0 will disable adaptation layer event notifications.
    adaptation_layer_event: u8,

    /// Setting this flag to 1 will enable the
    /// reception of authentication layer event notifications.  Setting
    /// the flag to 0 will disable authentication layer event
    /// notifications.
    authentication_event: u8,

    /// Setting this flag to 1 will enable the
    /// reception of sender dry event notifications.  Setting the flag to
    /// 0 will disable sender dry event notifications.
    sender_dry_event: u8,
}

/// The SCTP association status
///
/// Applications can retrieve current status information about an
/// association, including association state, peer receiver window size,
/// number of unacknowledged DATA chunks, and number of DATA chunks
/// pending receipt.  This information is read-only.
/// <https://www.rfc-editor.org/rfc/rfc6458#section-8.2.1>
#[derive(Clone, Debug)]
#[repr(C)]
pub struct Status {
    /// This parameter is ignored for one-to-one style
    /// sockets.  For one-to-many style sockets, it holds the identifier
    /// for the association.  All notifications for a given association
    /// have the same association identifier.  The special SCTP_{FUTURE|
    /// CURRENT|ALL}_ASSOC cannot be used.
    pub assoc_id: sctp_assoc_t,

    /// This contains the association's current state, i.e.,
    /// one of the following values:
    /// *  SCTP_CLOSED
    /// *  SCTP_BOUND
    /// *  SCTP_LISTEN
    /// *  SCTP_COOKIE_WAIT
    /// *  SCTP_COOKIE_ECHOED
    /// *  SCTP_ESTABLISHED
    /// *  SCTP_SHUTDOWN_PENDING
    /// *  SCTP_SHUTDOWN_SENT
    /// *  SCTP_SHUTDOWN_RECEIVED
    /// *  SCTP_SHUTDOWN_ACK_SENT
    pub state: i32,

    /// This contains the association peer's current receiver window size.
    pub rwnd: u32,

    /// This is the number of unacknowledged DATA chunks.
    pub unackdata: u16,

    /// This is the number of DATA chunks pending receipt.
    pub penddata: u16,

    /// This is the number of streams that the peer will be using outbound.
    pub instrms: u16,

    /// This is the number of outbound streams that the endpoint is allowed to use.
    pub outstrms: u16,

    /// This is the size at which SCTP fragmentation will occur.
    pub fragmentation_point: u32,

    /// This is information on the current primary peer address.
    pub primary: PeerAddrInfo,
}
