use core::pin::Pin;
use core::future::Future;
use core::task::{Context, Poll};

use super::AsyncFileIOExt;

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct SetCloseOnExecFuture<'a, T: Unpin + ?Sized> {
    pub(crate) file: &'a T,
    pub(crate) is_set: bool,
}

impl<T: AsyncFileIOExt + Unpin + ?Sized> Future for SetCloseOnExecFuture<'_, T> {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { file, is_set } = self.get_mut();
        Pin::new(&**file).set_close_on_exec(cx, *is_set)
    }
}