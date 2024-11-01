use std::{
    collections::VecDeque,
    io::{self, stderr, stdout, Read, Stderr, Stdout, Write},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};

#[cfg(test)]
use std::time::Duration;

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
        let result = self
            .inner
            .lock()
            .map_err(|_e| std::io::Error::other("Failed to lock mutex"))?
            .write(buf);
        self.notify.send(()).ok();
        result
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner
            .lock()
            .map_err(|_e| std::io::Error::other("Failed to lock mutex"))?
            .flush()
    }
}

struct TeeWrite<A: Write, B: Write> {
    a: A,
    b: B,
}

impl<A: Write, B: Write> Write for TeeWrite<A, B> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let len = buf.len();
        self.a.write_all(buf)?;
        self.b.write_all(buf)?;
        Ok(len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.a.flush()?;
        self.b.flush()
    }
}

pub struct MutexVecDequeRead {
    inner: Arc<Mutex<VecDeque<u8>>>,
    notify: Receiver<()>,
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

pub struct DeploymentHandle {
    inner_info: TeeWrite<MutexVecDequeWrite, Stdout>,
    inner_error: TeeWrite<MutexVecDequeWrite, Stderr>,
}
impl Clone for DeploymentHandle {
    fn clone(&self) -> Self {
        DeploymentHandle {
            inner_info: TeeWrite {
                a: self.inner_info.a.clone(),
                b: stdout(),
            },
            inner_error: TeeWrite {
                a: self.inner_error.a.clone(),
                b: stderr(),
            },
        }
    }
}

impl DeploymentHandle {
    pub fn info(&mut self) -> &mut dyn Write {
        &mut self.inner_info
    }
    pub fn error(&mut self) -> &mut dyn Write {
        &mut self.inner_error
    }
}

pub struct DeploymentLogs {
    inner_info: MutexVecDequeRead,
    inner_error: MutexVecDequeRead,
}

#[allow(unused)]
impl DeploymentLogs {
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

pub fn deployment_handle() -> (DeploymentHandle, DeploymentLogs) {
    let info = vec_deque_channel();
    let err = vec_deque_channel();

    (
        DeploymentHandle {
            inner_info: TeeWrite {
                a: info.0,
                b: stdout(),
            },
            inner_error: TeeWrite {
                a: err.0,
                b: stderr(),
            },
        },
        DeploymentLogs {
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
        let (mut stream, mut consumer) = deployment_handle();
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
        let (mut stream, mut consumer) = deployment_handle();

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
        let (mut stream, mut consumer) = deployment_handle();

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
