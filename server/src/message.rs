use std::{
    io::{self, Read},
    sync::{Arc, Mutex},
    task::Poll,
};

use pond_deployment::DeploymentLogs;
use rocket::{
    response::Responder,
    tokio::io::{AsyncRead, ReadBuf},
    Response,
};

pub struct AsyncLogStream {
    shared_state: Arc<Mutex<SharedState>>,
}
#[derive(Default)]
struct SharedState {
    buffer: Vec<u8>,
    waker: Option<std::task::Waker>,
    info_closed: bool,
    error_closed: bool,
}

impl AsyncLogStream {
    pub fn from_deployment_logs(deployment_logs: DeploymentLogs) -> Self {
        let (info, error) = deployment_logs.into_read();
        let shared_state: Arc<Mutex<SharedState>> = Default::default();
        let info_shared = shared_state.clone();
        let error_shared = shared_state.clone();
        std::thread::spawn(move || {
            let mut info = info;
            let mut buffer = vec![0; 4096];
            loop {
                let bytes_read = info.read(&mut buffer).unwrap();
                if bytes_read == 0 {
                    break;
                }

                let mut locked = info_shared.lock().unwrap();
                locked.buffer.extend_from_slice(&buffer[..bytes_read]);
                if let Some(waker) = locked.waker.take() {
                    waker.wake();
                }
            }
            info_shared.lock().unwrap().info_closed = true;
        });

        std::thread::spawn(move || {
            let mut error = error;
            let mut buffer = vec![0; 4096];
            loop {
                let bytes_read = error.read(&mut buffer).unwrap();
                if bytes_read == 0 {
                    break;
                }

                let mut locked = error_shared.lock().unwrap();
                locked.buffer.extend_from_slice(&buffer[..bytes_read]);
                if let Some(waker) = locked.waker.take() {
                    waker.wake();
                }
            }
            error_shared.lock().unwrap().error_closed = true;
        });

        Self { shared_state }
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
        let mut shared_state = self.shared_state.lock().unwrap();
        if !shared_state.buffer.is_empty() {
            let bytes_to_read = std::cmp::min(shared_state.buffer.len(), buf.remaining());
            buf.put_slice(&shared_state.buffer[..bytes_to_read]);
            shared_state.buffer.drain(..bytes_to_read);
            Poll::Ready(Ok(()))
        } else if shared_state.info_closed && shared_state.error_closed {
            Poll::Ready(Ok(()))
        } else {
            shared_state.waker = Some(_cx.waker().clone());
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Instant};

    use rocket::tokio::{self, io::AsyncReadExt};

    #[tokio::test]
    async fn test_async_log_stream() {
        let (mut handle, logs) = pond_deployment::deployment_handle();
        let jh = std::thread::spawn(move || {
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
        jh.join().unwrap();
        let bytes_read = stream.read(&mut buffer).await.unwrap();
        assert_eq!(bytes_read, 0);
    }
}
