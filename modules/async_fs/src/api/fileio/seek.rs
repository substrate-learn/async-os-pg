use core::pin::Pin;
use core::future::Future;
use core::task::{Context, Poll};

use super::AsyncFileIOExt;
use async_io::SeekFrom;
use axerrno::AxResult;

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct SeekFuture<'a, T: Unpin + ?Sized> {
    pub(crate) file: &'a T,
    pub(crate) pos: SeekFrom
}

impl<T: AsyncFileIOExt + Unpin + ?Sized> Future for SeekFuture<'_, T> {
    type Output = AxResult<u64>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { file, pos } = self.get_mut();
        Pin::new(&**file).seek(cx, *pos)
    }
}