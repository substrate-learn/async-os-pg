use crate::io::{self, prelude::*, AsyncBufReader};
use crate::sync::{Mutex, MutexGuardFuture};
use crate::io::AsyncBufReadExt;

#[cfg(feature = "alloc")]
use alloc::string::String;
use axio::{AsyncRead, AsyncWrite, AsyncWriteExt};
use core::task::Poll;
use core::task::Context;
use core::future::Future;
use core::pin::Pin;

struct StdinRaw;
struct StdoutRaw;

impl AsyncRead for StdinRaw {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let mut read_len = 0;
        while read_len < buf.len() {
            if let Some(c) = arceos_api::stdio::ax_console_read_byte() {
                buf[read_len] = c;
                read_len += 1;
            } else {
                break;
            }
        }
        Poll::Ready(Ok(read_len))
    }
}

// impl Read for StdinRaw {
//     // Non-blocking read, returns number of bytes read.
//     fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
//         let mut read_len = 0;
//         while read_len < buf.len() {
//             if let Some(c) = arceos_api::stdio::ax_console_read_byte() {
//                 buf[read_len] = c;
//                 read_len += 1;
//             } else {
//                 break;
//             }
//         }
//         Ok(read_len)
//     }
// }

impl AsyncWrite for StdoutRaw {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<axio::Result<usize>> {
        Poll::Ready(arceos_api::stdio::ax_console_write_bytes(buf))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<axio::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<axio::Result<()>> {
        self.poll_flush(cx)
    }
}

// impl Write for StdoutRaw {
//     fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
//         arceos_api::stdio::ax_console_write_bytes(buf)
//     }
//     fn flush(&mut self) -> io::Result<()> {
//         Ok(())
//     }
// }

/// A handle to the standard input stream of a process.
pub struct Stdin {
    inner: &'static Mutex<AsyncBufReader<StdinRaw>>,
}

/// A locked reference to the [`Stdin`] handle.
pub struct StdinLock<'a> {
    inner: MutexGuardFuture<'a, AsyncBufReader<StdinRaw>>,
}

impl Stdin {
    /// Locks this handle to the standard input stream, returning a readable
    /// guard.
    ///
    /// The lock is released when the returned lock goes out of scope. The
    /// returned guard also implements the [`Read`] and [`BufRead`] traits for
    /// accessing the underlying data.
    pub fn lock(&self) -> StdinLock<'static> {
        // Locks this handle with 'static lifetime. This depends on the
        // implementation detail that the underlying `Mutex` is static.
        StdinLock {
            inner: self.inner.lock(),
        }
    }

    /// Locks this handle and reads a line of input, appending it to the specified buffer.
    #[cfg(feature = "alloc")]
    pub fn read_line(&self, cx: &mut Context<'_>, buf: &mut String) -> Poll<io::Result<usize>> {
        if let Poll::Ready(mut inner) = Pin::new(&mut self.inner.lock())
            .as_mut()
            .poll(cx) {
                Pin::new(&mut inner.read_line(buf)).as_mut().poll(cx)
        } else {
            Poll::Pending
        }
    }
}

impl AsyncRead for Stdin {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        if let Poll::Ready(lock) = Pin::new(&mut self.inner.lock()).as_mut().poll(cx) {
            let read_len = lock.buffer().read(buf)?;
            if buf.is_empty() || read_len > 0 {
                Poll::Ready(Ok(read_len))
            } else {
                Poll::Pending
            }
        } else {
            Poll::Pending
        }
    }
}

// impl Read for Stdin {
//     // Block until at least one byte is read.
//     fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
//         let read_len = self.inner.lock().read(buf)?;
//         if buf.is_empty() || read_len > 0 {
//             return Ok(read_len);
//         }
//         // try again until we got something
//         loop {
//             let read_len = self.inner.lock().read(buf)?;
//             if read_len > 0 {
//                 return Ok(read_len);
//             }
//             crate::thread::yield_now();
//         }
//     }
// }

impl AsyncRead for StdinLock<'_> {
    // fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    //     self.inner.read(buf)
    // }
    fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).as_mut().poll(cx).map(|inner| {
            inner.buffer().read(buf)
        })
    }
}

// impl BufRead for StdinLock<'_> {
//     fn fill_buf(&mut self) -> io::Result<&[u8]> {
//         self.inner.fill_buf()
//     }

//     fn consume(&mut self, n: usize) {
//         self.inner.consume(n)
//     }

//     #[cfg(feature = "alloc")]
//     fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> io::Result<usize> {
//         self.inner.read_until(byte, buf)
//     }

//     #[cfg(feature = "alloc")]
//     fn read_line(&mut self, buf: &mut String) -> io::Result<usize> {
//         self.inner.read_line(buf)
//     }
// }

/// A handle to the global standard output stream of the current process.
pub struct Stdout {
    inner: &'static Mutex<StdoutRaw>,
}

/// A locked reference to the [`Stdout`] handle.
pub struct StdoutLock<'a> {
    inner: MutexGuardFuture<'a, StdoutRaw>,
}

impl Stdout {
    /// Locks this handle to the standard output stream, returning a writable
    /// guard.
    ///
    /// The lock is released when the returned lock goes out of scope. The
    /// returned guard also implements the `Write` trait for writing data.
    pub fn lock(&self) -> StdoutLock<'static> {
        StdoutLock {
            inner: self.inner.lock(),
        }
    }
}

// impl Write for Stdout {
//     fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
//         self.inner.lock().write(buf)
//     }
//     fn flush(&mut self) -> io::Result<()> {
//         self.inner.lock().flush()
//     }
// }

// impl Write for StdoutLock<'_> {
//     fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
//         self.inner.write(buf)
//     }
//     fn flush(&mut self) -> io::Result<()> {
//         self.inner.flush()
//     }
// }

/// Constructs a new handle to the standard input of the current process.
pub fn stdin() -> Stdin {
    static INSTANCE: Mutex<AsyncBufReader<StdinRaw>> = Mutex::new(AsyncBufReader::new(StdinRaw));
    Stdin { inner: &INSTANCE }
}

/// Constructs a new handle to the standard output of the current process.
pub fn stdout() -> Stdout {
    static INSTANCE: Mutex<StdoutRaw> = Mutex::new(StdoutRaw);
    Stdout { inner: &INSTANCE }
}

#[doc(hidden)]
pub fn __print_impl(args: core::fmt::Arguments) {
    if cfg!(feature = "smp") {
        // synchronize using the lock in axlog, to avoid interleaving
        // with kernel logs
        arceos_api::stdio::ax_console_write_fmt(args).unwrap();
    } else {
        let waker = core::task::Waker::noop();
        let mut cx = Context::from_waker(waker);
        let _ = Pin::new(&mut stdout().lock().inner).as_mut().poll(&mut cx).map(|mut r| {
            Pin::new(&mut r.write_fmt(args)).as_mut().poll(&mut cx).map(|r| r.unwrap())
        });
    }
}
