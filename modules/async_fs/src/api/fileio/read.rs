use core::pin::Pin;
use core::future::Future;
use core::task::{Context, Poll};

use super::AsyncFileIOExt;
use axerrno::AxResult;

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct ReadFuture<'a, T: Unpin + ?Sized> {
    pub(crate) file: &'a T,
    pub(crate) buf: &'a mut [u8], 
}

impl<T: AsyncFileIOExt + Unpin + ?Sized> Future for ReadFuture<'_, T> {
    type Output = AxResult<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { file, buf } = self.get_mut();
        Pin::new(&**file).read(cx, buf)
    }
}