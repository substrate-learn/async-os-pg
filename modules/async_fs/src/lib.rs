//! 定义了文件系统的实现。
//! 
//! dev.rs 中定义了块设备的驱动实现，包括设备大小、寻址位置、读写单个扇区、从指定扇区进行读写等操作
//! 
//! root.rs 中定义了文件系统根目录的实现，包括根目录的初始化、根目录的操作等。
//! 
//! fops.rs 中定义了 File、Directory、OpenOptions 等结构。
//!     1. File：其内部的实现为任意实现了 VfsNodeOps + Unpin trait 的对象，File 实现了 AsyncRead、AsyncWrite、AsyncSeek trait，提供了 IO 接口
//! 
//! 

#![cfg_attr(not(test), no_std)]
#![feature(async_iterator)]

#[macro_use]
extern crate log;
extern crate alloc;

mod fs;
mod dev;
mod root;
#[allow(unused)]
mod mounts;

pub mod api;
pub mod fops;
pub use fs::BLOCK_SIZE;


use axdriver::{prelude::*, AxDeviceContainer};

/// Initializes filesystems by block devices.
pub async fn init_filesystems(mut blk_devs: AxDeviceContainer<AxBlockDevice>) {
    info!("Initialize filesystems...");
    
    let dev = blk_devs.take_one().expect("No block device found!");
    info!("  use block device 0: {:?}", dev.device_name());
    self::root::init_rootfs(self::dev::Disk::new(dev)).await;
}