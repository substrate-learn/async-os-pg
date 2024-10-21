use core::pin::Pin;
use core::future::Future;
use core::task::{Context, Poll};

use super::AsyncFileIOExt;
use axerrno::AxResult;

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct TruncateFuture<'a, T: Unpin + ?Sized> {
    pub(crate) file: &'a T,
    pub(crate) len: usize,
}

impl<T: AsyncFileIOExt + Unpin + ?Sized> Future for TruncateFuture<'_, T> {
    type Output = AxResult<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { file, len } = self.get_mut();
        Pin::new(&**file).truncate(cx, *len)
    }
}