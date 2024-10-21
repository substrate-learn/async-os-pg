use core::pin::Pin;
use core::future::Future;
use core::task::{Context, Poll};

use super::AsyncFileIOExt;
use axerrno::AxResult;

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct FlushFuture<'a, T: Unpin + ?Sized> {
    pub(crate) file: &'a T,
}

impl<T: AsyncFileIOExt + Unpin + ?Sized> Future for FlushFuture<'_, T> {
    type Output = AxResult<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&*self.file).flush(cx)
    }
}