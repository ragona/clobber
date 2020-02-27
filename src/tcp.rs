use crate::config::Config;
use crossbeam_channel::{bounded, Receiver, TryRecvError};
use log::{debug, error, info, warn};
use std::net::SocketAddr;
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

/// Worker pool that handles incoming tcp requests
/// Open question how to best share bytes between separate threads
/// `Bytes` looks promising; cloneable handle?
///
/// Goal is to decouple this from the mutators generating the bytes
/// And ALSO keep it separate from the analysis happening to the return value
/// I suspect the analysis may be CPU intensive in a way that does not play
/// nicely with this worker loop, so I'd like to isolate it in a different os thread.
/// (Need to measure.)
///
pub async fn load(
    config: Config,
    work_rx: Receiver<()>,
    bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    info!("beginning tcp workers");
    info!("{:?}", config);

    for i in 0..config.workers {
        let rx = work_rx.clone();
        let bytes = bytes.to_vec();
        let config = config.clone();
        let mut read_buf = [0u8; 1024];

        tokio::spawn(async move {
            loop {
                if let Err(e) = rx.try_recv() {
                    if let TryRecvError::Disconnected = e {
                        info!("worker loop {} broken", i);
                        break;
                    }
                }

                match request(config.target, &bytes, &mut read_buf).await {
                    Ok(_) => {
                        // todo: Analysis thread needs access to read_buf
                        // Perhaps this should actually be heap allocated? Measure.
                    }
                    Err(e) => {
                        warn!("{}", e);
                    }
                }
            }
        });
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ConfigBuilder;

    #[tokio::test]
    pub async fn test_load() -> Result<(), Box<dyn std::error::Error>> {
        let config = ConfigBuilder::new("0.0.0.0:8000".parse().unwrap())
            .workers(1)
            .build();

        let (tx, rx) = bounded(10);

        for _ in 0..10 {
            tx.send(()).expect("Failed to send");
        }

        load(config, rx, b"foo").await?;

        Ok(())
    }
}
