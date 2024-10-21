use core::pin::Pin;
use core::future::Future;
use core::task::{Context, Poll};

use super::AsyncFileIOExt;
use crate::api::OpenFlags;

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct GetStatusFuture<'a, T: Unpin + ?Sized> {
    pub(crate) file: &'a T,
}

impl<T: AsyncFileIOExt + Unpin + ?Sized> Future for GetStatusFuture<'_, T> {
    type Output = OpenFlags;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&*self.file).get_status(cx)
    }
}