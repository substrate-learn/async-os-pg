use core::pin::Pin;
use core::future::Future;

use super::AsyncWrite as Write;
use core::task::{Context, Poll};
use crate::Result;

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct WriteFuture<'a, T: Unpin + ?Sized> {
    pub(crate) writer: &'a mut T,
    pub(crate) buf: &'a [u8],
}

impl<T: Write + Unpin + ?Sized> Future for WriteFuture<'_, T> {
    type Output = Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let buf = self.buf;
        Pin::new(&mut *self.writer).poll_write(cx, buf)
    }
}
