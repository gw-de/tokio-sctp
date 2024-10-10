use std::{env, error::Error};

use bytes::BytesMut;
use tokio_sctp::{InitMsg, SctpListener, SendOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addrs = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string())
        .split(",")
        .map(|s| s.parse().unwrap())
        .collect::<Vec<_>>();

    let listener = SctpListener::bindx(&addrs)?;
    listener.set_sctp_initmsg(&InitMsg {
        num_ostreams: 5,
        max_instreams: 5,
        max_attempts: 0,
        max_init_timeout: 0,
    })?;

    println!("Listening on: {:?}", addrs.as_slice());

    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let mut buf = BytesMut::with_capacity(1024);

            loop {
                let (n, _, _) = socket
                    .recvmsg_buf(&mut buf)
                    .await
                    .expect("failed to read data from socket");

                if n == 0 {
                    return;
                }

                socket
                    .sendmsg(&buf, None, &SendOptions::default())
                    .await
                    .expect("failed to write data to socket");
            }
        });
    }
}
