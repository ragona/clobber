use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::prelude::*;

pub async fn round_trip(addr: SocketAddr, data: &[u8], read_buf: &mut [u8]) {
    let stream = TcpStream::connect(addr).await;
    if let Err(e) = stream {
        dbg!(e);
        return;
    }

    let mut stream = stream.unwrap();

    match stream.write_all(data).await {
        Err(e) => {
            dbg!(e);
            return;
        }
        _ => {}
    }

    match stream.read(read_buf).await {
        Err(e) => {
            dbg!(e);
        }
        _ => {}
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    pub async fn foo() {
        let data = b"foo";
        let mut read_buf = [0u8; 16];

        round_trip("0.0.0.0:8000".parse().unwrap(), data, &mut read_buf).await;
    }
}
