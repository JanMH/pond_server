use std::{
    io::{self, Read},
    sync::{
        mpsc::{channel, Receiver},
        Mutex,
    },
    task::Poll,
    usize,
};

use pond_deployment::{DeploymentLogs, LogStream};
use rocket::{
    response::Responder,
    tokio::{
        self,
        io::{AsyncRead, ReadBuf}
    },
    Response,
};

enum LogType {
    Info(Vec<u8>),
    Error(Vec<u8>),
}

pub struct AsyncLogStream {
    receiver: Receiver<LogType>,
}

impl AsyncLogStream {
    pub fn from_deployment_logs(deployment_logs: DeploymentLogs) -> Self {
        let (info, error) = deployment_logs.into_read();
        let (sender, receiver) = channel();
        let err_sender = sender.clone();

        std::thread::spawn(move || {
            let mut info = info;
            let mut buffer = vec![0; 4096];
            loop {
                let bytes_read = info.read(&mut buffer).unwrap();
                if bytes_read == 0 {
                    break;
                }
                if let Err(_) = sender.send(LogType::Info(buffer[..bytes_read].to_vec())) {
                    break;
                }
            }
        });

        std::thread::spawn(move || {
            let mut error = error;
            let mut buffer = vec![0; 4096];
            loop {
                let bytes_read = error.read(&mut buffer).unwrap();
                if bytes_read == 0 {
                    break;
                }
                if let Err(_) = err_sender.send(LogType::Error(buffer[..bytes_read].to_vec())) {
                    break;
                }
            }
        });

        Self { receiver }
    }
}

impl<'r> Responder<'r, 'r> for AsyncLogStream {
    fn respond_to(self, _request: &'r rocket::Request<'_>) -> rocket::response::Result<'r> {
        Ok(Response::build().streamed_body(self).finalize())
    }
}

impl AsyncRead for AsyncLogStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        tokio::task::block_in_place(|| match self.receiver.recv() {
            Ok(LogType::Info(data)) | Ok(LogType::Error(data)) => {
                buf.put_slice(&data);
                Poll::Ready(Ok(()))
            }
            Err(_) => Poll::Ready(Ok(())),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Instant};

    use rocket::tokio::{self, io::AsyncReadExt};


    #[tokio::test]
    async fn test_async_log_stream() {
        let (mut handle, logs) = pond_deployment::deployment_handle();
        std::thread::spawn(move || {
            handle.info().write(&[0]).unwrap();
            thread::sleep(std::time::Duration::from_millis(300));
            handle.error().write(&[1]).unwrap();
            thread::sleep(std::time::Duration::from_millis(300));
            handle.info().write(&[2]).unwrap();
        });
        let mut stream = super::AsyncLogStream::from_deployment_logs(logs);
        let mut buffer = vec![0; 4096];
        let mut last_byte_read = Instant::now();
        for i in 0..3 {
            let bytes_read = stream.read(&mut buffer).await.unwrap();
            assert_eq!(bytes_read, 1);
            assert_eq!(buffer[0], i);
            assert!(last_byte_read.elapsed() < std::time::Duration::from_millis(500));
            last_byte_read = Instant::now();
        }
    }
}