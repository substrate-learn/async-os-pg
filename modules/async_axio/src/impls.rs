use crate::{prelude::*, IoSliceMut, Result};
use core::cmp;

impl Read for &[u8] {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let amt = cmp::min(buf.len(), self.len());
        let a = &self[..amt];
        let b = &self[amt..];

        // First check if the amount of bytes we want to read is small:
        // `copy_from_slice` will generally expand to a call to `memcpy`, and
        // for a single byte the overhead is significant.
        if amt == 1 {
            buf[0] = a[0];
        } else {
            buf[..amt].copy_from_slice(a);
        }

        *self = b;
        Ok(amt)
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        if buf.len() > self.len() {
            return axerrno::ax_err!(UnexpectedEof, "failed to fill whole buffer");
        }
        let amt = buf.len();
        let a = &self[..amt];
        let b = &self[amt..];

        // First check if the amount of bytes we want to read is small:
        // `copy_from_slice` will generally expand to a call to `memcpy`, and
        // for a single byte the overhead is significant.
        if amt == 1 {
            buf[0] = a[0];
        } else {
            buf[..amt].copy_from_slice(a);
        }

        *self = b;
        Ok(())
    }

    #[inline]
    #[cfg(feature = "alloc")]
    fn read_to_end(&mut self, buf: &mut alloc::vec::Vec<u8>) -> Result<usize> {
        buf.extend_from_slice(self);
        let len = self.len();
        *self = &self[len..];
        Ok(len)
    }

    #[inline]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
        let mut nread = 0;
        for buf in bufs {
            nread += self.read(buf)?;
            if self.is_empty() {
                break;
            }
        }

        Ok(nread)
    }

}


// impl Read for &[u8] {
//     #[inline]
//     fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
//         let amt = cmp::min(buf.len(), self.len());
//         let (a, b) = self.split_at(amt);

//         // First check if the amount of bytes we want to read is small:
//         // `copy_from_slice` will generally expand to a call to `memcpy`, and
//         // for a single byte the overhead is significant.
//         if amt == 1 {
//             buf[0] = a[0];
//         } else {
//             buf[..amt].copy_from_slice(a);
//         }

//         *self = b;
//         Ok(amt)
//     }

//     #[inline]
//     fn read_buf(&mut self, mut cursor: BorrowedCursor<'_>) -> io::Result<()> {
//         let amt = cmp::min(cursor.capacity(), self.len());
//         let (a, b) = self.split_at(amt);

//         cursor.append(a);

//         *self = b;
//         Ok(())
//     }

//     #[inline]
//     fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
//         let mut nread = 0;
//         for buf in bufs {
//             nread += self.read(buf)?;
//             if self.is_empty() {
//                 break;
//             }
//         }

//         Ok(nread)
//     }

//     #[inline]
//     fn is_read_vectored(&self) -> bool {
//         true
//     }

//     #[inline]
//     fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
//         if buf.len() > self.len() {
//             return Err(io::Error::READ_EXACT_EOF);
//         }
//         let (a, b) = self.split_at(buf.len());

//         // First check if the amount of bytes we want to read is small:
//         // `copy_from_slice` will generally expand to a call to `memcpy`, and
//         // for a single byte the overhead is significant.
//         if buf.len() == 1 {
//             buf[0] = a[0];
//         } else {
//             buf.copy_from_slice(a);
//         }

//         *self = b;
//         Ok(())
//     }

//     #[inline]
//     fn read_buf_exact(&mut self, mut cursor: BorrowedCursor<'_>) -> io::Result<()> {
//         if cursor.capacity() > self.len() {
//             return Err(io::Error::READ_EXACT_EOF);
//         }
//         let (a, b) = self.split_at(cursor.capacity());

//         cursor.append(a);

//         *self = b;
//         Ok(())
//     }

//     #[inline]
//     fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
//         let len = self.len();
//         buf.try_reserve(len)?;
//         buf.extend_from_slice(*self);
//         *self = &self[len..];
//         Ok(len)
//     }

//     #[inline]
//     fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
//         let content = str::from_utf8(self).map_err(|_| io::Error::INVALID_UTF8)?;
//         buf.push_str(content);
//         let len = self.len();
//         *self = &self[len..];
//         Ok(len)
//     }
// }