use core::pin::Pin;
use core::future::Future;
use core::task::{Context, Poll};

use super::AsyncFileIOExt;
use axerrno::AxResult;

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct IoCtlFuture<'a, T: Unpin + ?Sized> {
    pub(crate) file: &'a T,
    pub(crate) request: usize, 
    pub(crate) arg1: usize
}

impl<T: AsyncFileIOExt + Unpin + ?Sized> Future for IoCtlFuture<'_, T> {
    type Output = AxResult<isize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { file, request, arg1 } = self.get_mut();
        Pin::new(&**file).ioctl(cx, *request, *arg1)
    }
}