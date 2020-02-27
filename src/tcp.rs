use crate::Config;
use crossbeam_channel::{bounded, TryRecvError};
use std::net::SocketAddr;
use tokio::prelude::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub struct Settings {
    pub workers: usize,
}

pub async fn request(
    host: SocketAddr,
    body: &[u8],
    mut read_buf: &mut [u8],
) -> tokio::io::Result<()> {
    let mut stream = match TcpStream::connect(host).await {
        Ok(s) => s,
        Err(e) => return Err(e),
    };

    stream.write(body).await?;
    stream.read(&mut read_buf).await?;

    Ok(())
}

async fn load(settings: Config, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let (work_tx, work_rx) = bounded(10);

    for i in 0..settings.workers {
        let rx = work_rx.clone();
        let bytes = bytes.to_vec();
        let settings = settings.clone();
        let mut read_buf = [0u8; 1024];

        tokio::spawn(async move {
            loop {
                if let Err(e) = rx.try_recv() {
                    if let TryRecvError::Disconnected = e {
                        break;
                    }
                }

                match request(settings.target, &bytes, &mut read_buf).await {
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
        });
    }

    loop {
        match work_tx.send(()) {
            Ok(x) => {}
            Err(e) => {}
        }
    }

    Ok(())
}

pub async fn tcp_write_read(addr: SocketAddr, data: &[u8], read_buf: &mut [u8]) {
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

    #[tokio::test]
    pub async fn foo() {
        //
    }
}
