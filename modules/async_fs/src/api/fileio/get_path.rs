use core::pin::Pin;
use core::future::Future;
use core::task::{Context, Poll};
use alloc::string::String;
use super::AsyncFileIOExt;

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct GetPathFuture<'a, T: Unpin + ?Sized> {
    pub(crate) file: &'a T,
}

impl<T: AsyncFileIOExt + Unpin + ?Sized> Future for GetPathFuture<'_, T> {
    type Output = String;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&*self.file).get_path(cx)
    }
}