//! 该模块在 starry 仓库的 taskctx 模块的基础上，
//! 将与上下文切换相关的部分移除，仅仅用于记录任务的一些其他信息。
//! 因为目标是使用协程来作为最小的任务单元，协程以协作式的方式调度为主
//! 要为协程支持抢占式调度，不同的方式实现的抢占式调度，对上下文切换的处理不同，
//! 因此在这个模块中要穷尽这些切换方式不能很好的保证通用性，所以将这部分内容移除。
#![no_std]
#![feature(asm_const)]
extern crate alloc;

mod stat;
pub use stat::*;

mod task;
pub use task::*;

mod kstack;
pub use kstack::*;

mod current;
pub use current::*;

/// Disables kernel preemption.
///
/// It will increase the preemption disable counter of the current task.
#[cfg(feature = "preempt")]
pub fn disable_preempt() {
    let ptr: *const TaskInner = current_task_ptr();
    if !ptr.is_null() {
        unsafe {
            (*ptr).disable_preempt();
        }
    }
}

/// Enables kernel preemption.
///
/// It will decrease the preemption disable counter of the current task.Once the counter is zero, the
/// task can be preempted.
#[cfg(feature = "preempt")]
pub fn enable_preempt() {
    let ptr: *const TaskInner = current_task_ptr();
    if !ptr.is_null() {
        unsafe {
            (*ptr).enable_preempt();
        }
    }
}
