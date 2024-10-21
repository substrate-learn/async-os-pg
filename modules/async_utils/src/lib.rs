//! 这个库用于提供同步与异步之间的兼容层函数，
//! 参考了 core::future::poll_fn 的实现

#![no_std]
#![feature(noop_waker)]

use core::{fmt, future::Future, pin::Pin, task::{Context, Poll}};
pub use async_main::async_main;
// use core::future::poll_fn;

pub fn poll_fn<T, F>(f: F) -> PollFn<F>
where
    F: FnMut() -> Poll<T>,
{
    PollFn { f }
}

pub struct PollFn<F> {
    f: F,
}

impl<F: Unpin> Unpin for PollFn<F> {}

impl<F> fmt::Debug for PollFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PollFn").finish()
    }
}

impl<T, F> Future for PollFn<F>
where
    F: FnMut() -> Poll<T>,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<T> {
        // SAFETY: We are not moving out of the pinned field.
        (unsafe { &mut self.get_unchecked_mut().f })()
    }
}

use sync::Mutex;


/// 在 async 函数中使用 Mutex
pub async fn test() -> i32 {
    let a = Mutex::new(98);
    let b = a.lock().await;
    *b
}

/// Extracts the successful type of a `Poll<T>`.
///
/// This macro bakes in propagation of `Pending` signals by returning early.
#[macro_export]
macro_rules! ready {
    ($e:expr $(,)?) => {
        match $e {
            core::task::Poll::Ready(t) => t,
            core::task::Poll::Pending => return core::task::Poll::Pending,
        }
    };
}

/// Extracts the successful type of a `Poll<T>`.
///
/// This macro bakes in propagation of `Pending` signals by returning early.
#[macro_export]
macro_rules! poll {
    ($e:expr $(,)?) => {
        core::task::Poll::Ready($e)
    };
}

