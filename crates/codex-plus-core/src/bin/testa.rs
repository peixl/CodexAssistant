use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addrs = vec![
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::from_str("192.168.12.25").unwrap(), // machine IP
        IpAddr::from_str("127.0.0.2").unwrap(),
        IpAddr::from_str("127.127.127.127").unwrap(),
    ];

    for addr in addrs {
        println!("Trying {}", addr);
        let listener = match tokio::net::TcpListener::bind((addr, 0)).await {
            Ok(l) => l,
            Err(e) => {
                println!("Bind failed for {}: {}", addr, e);
                continue;
            }
        };
        let port = listener.local_addr()?.port();
        println!("Listening on {}", port);

        let server_addr = addr;
        let server = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let _ = stream.write_all(b"ok").await;
                let _ = stream.shutdown().await;
            }
        });

        let probe = async {
            let mut stream = tokio::net::TcpStream::connect((server_addr, port)).await?;
            let mut buf = [0u8; 2];
            stream.read_exact(&mut buf).await?;
            anyhow::Ok(())
        };

        let outcome = tokio::time::timeout(std::time::Duration::from_millis(1500), probe).await;
        server.abort();

        match outcome {
            Ok(Ok(())) => {
                println!("Success with {}", addr);
            }
            Ok(Err(error)) => {
                println!("Error with {}: {}", addr, error);
            }
            Err(_) => {
                println!("Timeout with {}", addr);
            }
        }
    }

    Ok(())
}
