//! 这个模块定义了与 IO 相关的接口
//! 由于直接从底层提供了这些接口，因此不再需要依赖原本的同步接口
//! 定义了 AsyncRead、AsyncWrite、AsyncSeek 这三个基础 trait，
//! 只需要实现这三个基础 trait，即可使用更多的高级接口（提供了默认实现）
//! AsyncRead -> Read
//!     1. read
//!     2. read_exact
//!     3. read_to_end
//!     4. read_to_string
//!     5. read_vectored
//! AsyncWrite -> Write
//!     1. write
//!     2. flush
//!     3. write_all
//!     4. write_fmt
//!     5. write_vectored
//! AsyncSeek -> Seek
//!     1. seek
//! 
#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, feature(noop_waker))]

extern crate alloc;

pub use axerrno::{AxError as Error, AxResult as Result, ax_err};

mod read;
mod write;
mod seek;
mod buf_read;
pub mod ioslice;
mod stream;
mod buf_reader;
mod buf_writer;
mod cursor;
pub mod prelude;

pub use ioslice::*;

pub use read::{AsyncRead, Read};
pub use buf_read::{AsyncBufRead, BufRead};
pub use write::{AsyncWrite, Write};
pub use seek::{AsyncSeek, Seek, SeekFrom};

pub use buf_reader::BufReader;
pub use buf_writer::BufWriter;
pub use stream::*;
pub use cursor::Cursor;



/// I/O poll results.
#[derive(Debug, Default, Clone, Copy)]
pub struct PollState {
    /// Object can be read now.
    pub readable: bool,
    /// Object can be writen now.
    pub writable: bool,
}