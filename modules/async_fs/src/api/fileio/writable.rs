use core::pin::Pin;
use core::future::Future;
use core::task::{Context, Poll};

use super::AsyncFileIOExt;

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct WritableFuture<'a, T: Unpin + ?Sized> {
    pub(crate) file: &'a T,
}

impl<T: AsyncFileIOExt + Unpin + ?Sized> Future for WritableFuture<'_, T> {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&*self.file).writable(cx)
    }
}