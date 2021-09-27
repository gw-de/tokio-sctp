use bytes::BytesMut;
use socket2::Domain;
use std::{io, net::ToSocketAddrs};
use tokio_sctp::{InitMsg, SctpListener, SctpSocket, SctpStream, SendOptions};

#[tokio::test]
async fn sendmsg_recvmsg_buf() -> io::Result<()> {
    let msg: &[u8] = b"hello";

    let listener = SctpListener::bind("127.0.0.1:0".parse().unwrap())?;
    let sender = SctpStream::connect(listener.local_addr().unwrap()).await?;

    let (receiver, _addr) = listener.accept().await.unwrap();

    sender.sendmsg(msg, None, &SendOptions::default()).await?;

    let mut recv_buf = BytesMut::with_capacity(msg.len());
    let (len, info, _flags) = receiver.recvmsg_buf(&mut recv_buf).await?;

    assert_eq!(0, info.stream);
    assert_eq!(msg.len(), len);
    assert_eq!(msg, &recv_buf[..]);
    Ok(())
}

#[tokio::test]
async fn recvmsg_buf_full() -> io::Result<()> {
    let msg: &[u8] = b"hello";

    let listener = SctpListener::bind("127.0.0.1:0".parse().unwrap())?;
    let sender = SctpStream::connect(listener.local_addr().unwrap()).await?;

    let (receiver, _addr) = listener.accept().await.unwrap();

    sender.sendmsg(msg, None, &SendOptions::default()).await?;

    let mut recv_buf = BytesMut::with_capacity(msg.len());
    let (len, info, _flags) = receiver.recvmsg_buf(&mut recv_buf).await?;

    assert_eq!(0, info.stream);
    assert_eq!(msg.len(), len);
    assert_eq!(msg, &recv_buf[..]);

    let msg_2: &[u8] = b" world";
    sender.sendmsg(msg_2, None, &SendOptions::default()).await?;

    // expect recv_buf to allocate more memory to fit the message
    let (len, info, _flags) = receiver.recvmsg_buf(&mut recv_buf).await?;

    assert_eq!(0, info.stream);
    assert_eq!(msg_2.len(), len);
    assert_eq!([msg, msg_2].concat(), &recv_buf[..]);
    Ok(())
}

#[tokio::test]
async fn initmsg() -> io::Result<()> {
    let msg: &[u8] = b"hello";

    let listener = SctpListener::bind("127.0.0.1:0".parse().unwrap())?;
    listener.set_sctp_initmsg(&InitMsg {
        num_ostreams: 3,
        max_instreams: 3,
        max_attempts: 3,
        max_init_timeout: 30,
    })?;

    let sender = {
        let socket = SctpSocket::new(Domain::IPV4)?;
        socket.set_sctp_initmsg(&InitMsg {
            num_ostreams: 2,
            max_instreams: 2,
            max_attempts: 3,
            max_init_timeout: 30,
        })?;
        socket.connect(listener.local_addr()?).await?
    };

    let (_receiver, _addr) = listener.accept().await.unwrap();

    let result = sender
        .sendmsg(
            msg,
            None,
            &SendOptions {
                stream: 2,
                ..Default::default()
            },
        )
        .await;

    // sent on invalid stream, expect error
    assert_eq!(
        result.map_err(|e| e.kind()),
        Err(io::ErrorKind::InvalidInput)
    );

    let result = sender
        .sendmsg(
            msg,
            None,
            &SendOptions {
                stream: 1,
                ..Default::default()
            },
        )
        .await?;

    assert_eq!(msg.len(), result);

    Ok(())
}

#[tokio::test]
async fn multi_homing() -> io::Result<()> {
    let msg: &[u8] = b"hello";

    let listener = SctpListener::bindx(&[
        "127.0.0.1:0".to_socket_addrs().unwrap().next().unwrap(),
        "127.0.0.2:0".to_socket_addrs().unwrap().next().unwrap(),
    ])?;
    let sender = SctpStream::connectx(&[listener.local_addr().unwrap()]).await?;

    let (receiver, _addr) = listener.accept().await.unwrap();

    sender.sendmsg(msg, None, &SendOptions::default()).await?;

    let mut recv_buf = BytesMut::with_capacity(msg.len());
    let (len, info, _flags) = receiver.recvmsg_buf(&mut recv_buf).await?;

    assert_eq!(0, info.stream);
    assert_eq!(msg.len(), len);
    assert_eq!(msg, &recv_buf[..]);
    Ok(())
}
