use core::pin::Pin;
use core::future::Future;

use super::{AsyncSeek, SeekFrom};
use core::task::{Context, Poll};
use crate::Result;

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct SeekFuture<'a, T: Unpin + ?Sized> {
    pub(crate) seeker: &'a mut T,
    pub(crate) pos: SeekFrom,
}

impl<T: AsyncSeek + Unpin + ?Sized> Future for SeekFuture<'_, T> {
    type Output = Result<u64>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pos = self.pos;
        Pin::new(&mut *self.seeker).poll_seek(cx, pos)
    }
}
