use core::pin::Pin;

use super::AsyncRead;
use crate::stream::Stream;
use crate::{Error, Result};
use core::task::{Context, Poll};

/// A stream over `u8` values of a reader.
///
/// This struct is generally created by calling [`bytes`] on a reader.
/// Please see the documentation of [`bytes`] for more details.
///
/// [`bytes`]: trait.Read.html#method.bytes
#[derive(Debug)]
pub struct Bytes<T> {
    pub(crate) inner: T,
}

impl<T: AsyncRead + Unpin> Stream for Bytes<T> {
    type Item = Result<u8>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut byte = 0;

        let rd = Pin::new(&mut self.inner);
        match futures_core::ready!(rd.poll_read(cx, core::slice::from_mut(&mut byte))) {
            Ok(0) => Poll::Ready(None),
            Ok(..) => Poll::Ready(Some(Ok(byte))),
            Err(ref e) if *e == Error::Interrupted => Poll::Pending,
            Err(e) => Poll::Ready(Some(Err(e))),
        }
    }
}

// #[cfg(all(test, feature = "default", not(target_arch = "wasm32")))]
// mod tests {
//     use crate::io;
//     use crate::prelude::*;
//     use crate::task;

//     #[test]
//     fn test_bytes_basics() -> std::io::Result<()> {
//         task::block_on(async move {
//             let raw: Vec<u8> = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];
//             let source: io::Cursor<Vec<u8>> = io::Cursor::new(raw.clone());

//             let mut s = source.bytes();

//             // TODO(@dignifiedquire): Use collect, once it is stable.
//             let mut result = Vec::new();
//             while let Some(byte) = s.next().await {
//                 result.push(byte?);
//             }

//             assert_eq!(result, raw);

//             Ok(())
//         })
//     }
// }
