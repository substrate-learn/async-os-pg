use core::pin::Pin;
use core::future::Future;
use core::task::{Context, Poll};

use crate::api::OpenFlags;

use super::AsyncFileIOExt;

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct SetStatusFuture<'a, T: Unpin + ?Sized> {
    pub(crate) file: &'a T,
    pub(crate) flags: OpenFlags
}

impl<T: AsyncFileIOExt + Unpin + ?Sized> Future for SetStatusFuture<'_, T> {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { file, flags } = self.get_mut();
        Pin::new(&**file).set_status(cx, *flags)
    }
}