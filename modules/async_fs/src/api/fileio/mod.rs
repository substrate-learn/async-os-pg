mod get_path;
mod get_stat;
mod get_status;
mod get_type;
mod in_exceptional_conditions;
mod ioctl;
mod is_hang_up;
mod print_content;
mod ready_to_read;
mod ready_to_write;
mod set_close_on_exec;
mod set_status;
mod truncate;

mod read;
mod write;
mod flush;
mod seek;

mod readable;
mod writable;
mod executable;

pub use get_path::GetPathFuture;
pub use get_type::GetTypeFuture;
pub use get_stat::GetStatFuture;
pub use truncate::TruncateFuture;
pub use print_content::PrintContentFuture;
pub use set_status::SetStatusFuture;
pub use get_status::GetStatusFuture;
pub use set_close_on_exec::SetCloseOnExecFuture;
pub use in_exceptional_conditions::InExceptionalConditionsFuture;
pub use is_hang_up::IsHangUpFuture;
pub use ready_to_read::ReadyToReadFuture;
pub use ready_to_write::ReadyToWriteFuture;
pub use ioctl::IoCtlFuture;
pub use read::ReadFuture;
pub use write::WriteFuture;
pub use flush::FlushFuture;
pub use seek::SeekFuture;
pub use readable::ReadableFuture;
pub use writable::WritableFuture;
pub use executable::ExecutableFuture;

use async_io::SeekFrom;
use super::{AsyncFileIOExt, OpenFlags};

/// File I/O trait. 文件I/O操作，用于设置文件描述符，值得注意的是，这里的read/write/seek都是不可变引用
///
/// 因为文件描述符读取的时候，是用到内部File成员的读取函数，自身应当为不可变，从而可以被Arc指针调用
pub trait FileIOExt: AsyncFileIOExt {
    
    /// 读取操作
    fn read<'a>(&'a self, buf: &'a mut [u8]) -> ReadFuture<'a, Self>
    where
        Self: Unpin,
    {
        ReadFuture { file: self, buf }
    }

    /// 写入操作
    fn write<'a>(&'a self, buf: &'a [u8]) -> WriteFuture<'a, Self>
    where
        Self: Unpin,
    {
        WriteFuture { file: self, buf }
    }

    /// 刷新操作
    fn flush<'a>(&'a self) -> FlushFuture<'a, Self>
    where
        Self: Unpin,
    {
        FlushFuture { file: self }
    }

    /// 移动指针操作
    fn seek<'a>(&'a self, pos: SeekFrom) -> SeekFuture<'a, Self>
    where
        Self: Unpin,
    {
        SeekFuture { file: self, pos }
    }

    /// whether the file is readable
    fn readable<'a>(&'a self) -> ReadableFuture<'a, Self> 
    where
        Self: Unpin,
    {
        ReadableFuture { file: self }
    }

    /// whether the file is writable
    fn writable<'a>(&'a self) -> WritableFuture<'a, Self> 
    where
        Self: Unpin,
    {
        WritableFuture { file: self }
    }

    /// whether the file is executable
    fn executable<'a>(&'a self) -> ExecutableFuture<'a, Self> 
    where
        Self: Unpin,
    {
        ExecutableFuture { file: self }
    }

    /// 获取类型
    fn get_type<'a>(&'a self) -> GetTypeFuture<'a, Self>
    where
        Self: Unpin,
    {
        GetTypeFuture { file: self }
    }

    /// 获取路径
    fn get_path<'a>(&'a self) -> GetPathFuture<'a, Self>
    where
        Self: Unpin,
    {
        GetPathFuture { file: self }
    }

    /// 获取文件信息
    fn get_stat<'a>(&'a self) -> GetStatFuture<'a, Self>
    where
        Self: Unpin,
    {
        GetStatFuture { file: self }
    }
    
    /// 截断文件到指定长度
    fn truncate<'a>(&'a self, len: usize) -> TruncateFuture<'a, Self>
    where
        Self: Unpin,
    {
        TruncateFuture { file: self, len }
    }

    /// debug
    fn print_content<'a>(&'a self) -> PrintContentFuture<'a, Self> 
    where
        Self: Unpin,
    {
        PrintContentFuture { file: self }
    }

    /// 设置文件状态
    fn set_status<'a>(&'a self, flags: OpenFlags) -> SetStatusFuture<'a, Self>
    where
        Self: Unpin,
    {
        SetStatusFuture { file: self, flags }
    }

    /// 获取文件状态
    fn get_status<'a>(&'a self) -> GetStatusFuture<'a, Self>
    where
        Self: Unpin,
    {
        GetStatusFuture { file: self }
    }

    /// 设置 close_on_exec 位
    /// 设置成功返回false
    fn set_close_on_exec<'a>(&'a self, is_set: bool) -> SetCloseOnExecFuture<'a, Self>
    where
        Self: Unpin,
    {
        SetCloseOnExecFuture { file: self, is_set }
    }

    /// 处于“意外情况”。在 (p)select 和 (p)poll 中会使用到
    ///
    /// 当前基本默认为false
    fn in_exceptional_conditions<'a>(&'a self) -> InExceptionalConditionsFuture<'a, Self>
    where
        Self: Unpin,
    {
        InExceptionalConditionsFuture { file: self }
    }

    /// 是否已经终止，对pipe来说相当于另一端已经关闭
    ///
    /// 对于其他文件类型来说，是在被close的时候终止，但这个时候已经没有对应的filedesc了，所以自然不会调用这个函数
    fn is_hang_up<'a>(&'a self) -> IsHangUpFuture<'a, Self> 
    where
        Self: Unpin,
    {
        IsHangUpFuture { file: self }
    }

    /// 已准备好读。对于 pipe 来说，这意味着读端的buffer内有值
    fn ready_to_read<'a>(&'a self) -> ReadyToReadFuture<'a, Self> 
    where
        Self: Unpin,
    {
        ReadyToReadFuture { file: self }
    }
    /// 已准备好写。对于 pipe 来说，这意味着写端的buffer未满
    fn ready_to_write<'a>(&'a self) -> ReadyToWriteFuture<'a, Self> 
    where
        Self: Unpin,
    {
        ReadyToWriteFuture { file: self }
    }

    /// To control the file descriptor
    fn ioctl<'a>(&'a self, request: usize, arg1: usize) -> IoCtlFuture<'a, Self> 
    where
        Self: Unpin,
    {
        IoCtlFuture { file: self, request, arg1 }
    }
}

impl<T: AsyncFileIOExt + ?Sized> FileIOExt for T {}