//! 以协程的方式实现同步原语、以及任务调度模块中的 WaitQueue、TimerQueue
//! 目前支持的原语：
//! - [`Mutex`]: A mutual exclusion primitive.

#![no_std]

extern crate alloc;

mod wait_list;
mod wait_queue;
mod mutex;
#[cfg(feature = "irq")]
mod timers;

pub use wait_queue::WaitQueue;
pub use mutex::*;
#[cfg(feature = "irq")]
pub use timers::{cancel_alarm, check_events, set_alarm_wakeup, init};



