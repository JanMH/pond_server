use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    sync::{
        mpsc::{channel, Receiver, Sender, TryRecvError},
        Arc, Mutex,
    },
    task::Poll
};

#[cfg(test)]
use std::time::Duration;

use rocket::tokio::{io::{AsyncRead, ReadBuf}, task::block_in_place};

use rocket::{
    response::Responder,
    Response,
};

#[derive(Clone)]
struct MutexVecDequeWrite {
    inner: Arc<Mutex<VecDeque<u8>>>,
    notify: Sender<()>,
}

impl Drop for MutexVecDequeWrite {
    fn drop(&mut self) {
        self.notify.send(()).ok();
    }
}

impl Write for MutexVecDequeWrite {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner
            .lock()
            .map_err(|_e| std::io::Error::other("Failed to lock mutex"))?
            .write(buf)
            .inspect(|_b| {
                self.notify.send(()).ok();
            })
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner
            .lock()
            .map_err(|_e| std::io::Error::other("Failed to lock mutex"))?
            .flush()
    }
}

pub struct MutexVecDequeRead {
    inner: Arc<Mutex<VecDeque<u8>>>,
    notify: Receiver<()>,
}

impl AsyncRead for MutexVecDequeRead {
     fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf,
    ) -> std::task::Poll<io::Result<()>> {
        {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_e| io::Error::other("Mutex is poisoned"))?;
        if !inner.is_empty() {
            let (left, _) = inner.as_slices();
            let len = left.len().min(buf.remaining());
            buf.put_slice(&left[0..len]);
            inner.drain(0..len);
            return Poll::Ready(Ok(()));
        }
        }
        
        if self.notify.try_recv() == Err(TryRecvError::Disconnected) {
            return Poll::Ready(Ok(()));
        }
        
        let waker = cx.waker().clone();
        block_in_place(move ||{
            self.notify.recv().ok();
            waker.wake();
            self.poll_read(cx, buf)
        })
        
    }
}

impl MutexVecDequeRead {
    fn wait_receive(&mut self) -> io::Result<bool> {
        // Block until some data is availabe to avoid endless spinning
        let mut is_empty = true;

        loop {
            match self.inner.try_lock() {
                Ok(deque) => {
                    is_empty = deque.is_empty();
                }
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    Err(std::io::Error::other("Mutex is poisoned"))?
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    return Ok(true);
                }
            }

            if is_empty {
                #[cfg(test)]
                match self.notify.recv_timeout(Duration::from_secs(3)) {
                    Ok(_) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        panic!("Had to wait for too long for timeout")
                    }
                    Err(_) => return Ok(true),
                }
                #[cfg(not(test))]
                match self.notify.recv() {
                    Ok(_) => {}
                    Err(_) => return Ok(true),
                }
            } else {
                return Ok(true);
            }
        }
    }
}

impl Read for MutexVecDequeRead {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.wait_receive()? {
            true => {}
            false => return Ok(0),
        }
        self.inner
            .lock()
            .map_err(|_e| std::io::Error::other("Failed to lock mutex"))?
            .read(buf)
    }
}

#[derive(Clone)]
pub struct MessageSender {
    inner_info: MutexVecDequeWrite,
    inner_error: MutexVecDequeWrite,
}

impl MessageSender {
    pub fn info(&mut self) -> &mut dyn Write {
        &mut self.inner_info
    }
    pub fn error(&mut self) -> &mut dyn Write {
        &mut self.inner_error
    }
}

pub struct MessageReceiver {
    inner_info: MutexVecDequeRead,
    inner_error: MutexVecDequeRead,
}

#[allow(unused)]
impl MessageReceiver {
    pub fn info(&mut self) -> &mut dyn Read {
        &mut self.inner_info
    }

    pub fn into_read(self) -> (MutexVecDequeRead, MutexVecDequeRead) {
        (self.inner_info, self.inner_error)
    }

    pub fn error(&mut self) -> &mut dyn Read {
        &mut self.inner_error
    }
}

impl<'r> Responder<'r, 'r> for MessageReceiver {
    fn respond_to(self, _request: &'r rocket::Request<'_>) -> rocket::response::Result<'r> {
        Ok(Response::build()
            .streamed_body(self.into_read().0)
            .finalize())
    }
}

fn vec_deque_channel() -> (MutexVecDequeWrite, MutexVecDequeRead) {
    let (s, r) = channel();
    let m: Arc<Mutex<VecDeque<u8>>> = Default::default();

    (
        MutexVecDequeWrite {
            inner: m.clone(),
            notify: s,
        },
        MutexVecDequeRead {
            inner: m,
            notify: r,
        },
    )
}

pub fn message_channel() -> (MessageSender, MessageReceiver) {
    let info = vec_deque_channel();
    let err = vec_deque_channel();

    (
        MessageSender {
            inner_info: info.0,
            inner_error: err.0,
        },
        MessageReceiver {
            inner_info: info.1,
            inner_error: err.1,
        },
    )
}

#[cfg(test)]
mod test {
    use super::*;
    use std::thread;
    use std::time::Duration;
    #[test]
    fn test_write_info_twice() {
        let (mut stream, mut consumer) = message_channel();
        let mut clone = stream.clone();

        let mut output: VecDeque<u8> = VecDeque::new();
        let r_ref = &mut output;
        std::thread::scope(move |s| {
            s.spawn(move || {
                std::io::copy(&mut consumer.inner_info, r_ref).unwrap();
            });
            thread::sleep(Duration::from_millis(50));
            write!(stream.info(), "Hello ").unwrap();
            write!(clone.info(), "World").unwrap();
            write!(stream.info(), "!").unwrap();
        });

        let mut result = String::new();
        output.read_to_string(&mut result).unwrap();

        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_close_streams_first() {
        let (mut stream, mut consumer) = message_channel();

        let mut output: VecDeque<u8> = VecDeque::new();
        let r_ref = &mut output;
        let t = std::thread::spawn(move || {
            write!(stream.info(), "Hello").unwrap();
            write!(stream.info(), "!").unwrap();
        });

        t.join().expect("Could not join test thread");

        std::io::copy(&mut consumer.inner_info, r_ref).unwrap();

        let mut result = String::new();
        output.read_to_string(&mut result).unwrap();

        assert_eq!(result, "Hello!");
    }

    #[test]
    fn test_close_streams_last() {
        let (mut stream, mut consumer) = message_channel();

        let mut output: VecDeque<u8> = VecDeque::new();
        let r_ref = &mut output;
        let t = std::thread::spawn(move || {
            write!(stream.info(), "Hello").unwrap();
            write!(stream.info(), "!").unwrap();
            thread::sleep(Duration::from_millis(300));
        });

        std::io::copy(&mut consumer.inner_info, r_ref).unwrap();

        t.join().expect("Could not join test thread");

        let mut result = String::new();
        output.read_to_string(&mut result).unwrap();

        assert_eq!(result, "Hello!");
    }
}
