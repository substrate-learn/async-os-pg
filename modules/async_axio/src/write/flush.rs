use core::pin::Pin;
use core::future::Future;

use super::AsyncWrite as Write;
use core::task::{Context, Poll};
use crate::Result;

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct FlushFuture<'a, T: Unpin + ?Sized> {
    pub(crate) writer: &'a mut T,
}

impl<T: Write + Unpin + ?Sized> Future for FlushFuture<'_, T> {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut *self.writer).poll_flush(cx)
    }
}
