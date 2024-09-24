#![cfg_attr(not(test), no_std)]
#![feature(doc_cfg)]
#![feature(doc_auto_cfg)]
#![feature(stmt_expr_attributes)]
#![feature(asm_const)]
#![feature(noop_waker)]

extern crate alloc;
#[macro_use]
extern crate log;

mod processor;
mod executor;
mod task;
mod api;
pub mod schedule;
mod stack_pool;
mod waker;

pub use api::*;
use executor::*;
pub use processor::*;

/// The reference type of a task.
pub type AxTaskRef = alloc::sync::Arc<AxTask>;
use crate::task::ScheduleTask;
pub use task::TaskState;

cfg_if::cfg_if! {
    if #[cfg(feature = "sched_rr")] {
        const MAX_TIME_SLICE: usize = 5;
        pub(crate) type AxTask = scheduler::RRTask<ScheduleTask, MAX_TIME_SLICE>;
        pub(crate) type Scheduler = scheduler::RRScheduler<ScheduleTask, MAX_TIME_SLICE>;
    } else if #[cfg(feature = "sched_cfs")] {
        pub(crate) type AxTask = scheduler::CFSTask<ScheduleTask>;
        pub(crate) type Scheduler = scheduler::CFScheduler<ScheduleTask>;
    } else if #[cfg(feature = "sched_moic")] {
        pub(crate) type AxTask = scheduler::MOICTask<ScheduleTask>;
        pub(crate) type Scheduler = scheduler::MOICScheduler<ScheduleTask>;
    } else {
        // If no scheduler features are set, use FIFO as the default.
        pub(crate) type AxTask = scheduler::FifoTask<ScheduleTask>;
        pub(crate) type Scheduler = scheduler::FifoScheduler<ScheduleTask>;
    }
}