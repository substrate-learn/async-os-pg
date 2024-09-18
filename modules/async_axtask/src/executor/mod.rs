pub(crate) mod executor;
pub(crate) mod kexecutor;
pub(crate) mod current;
pub(crate) use executor::*;


use kexecutor::KERNEL_EXECUTOR;
use core::{future::Future, pin::Pin};
use alloc::{string::ToString, boxed::Box, sync::Arc};
use crate::task::new_task;


type BoxFut = Pin<Box<dyn Future<Output = i32> + Send + 'static>>;
extern "C" { static ASYNC_MAIN: usize; }

extern "C" {
    fn main_fut() -> i32;
}

pub(crate) fn init() {
    let kexecutor = Arc::new(Executor::new());
    KERNEL_EXECUTOR.init_by(kexecutor.clone());
    unsafe { 
        // let main_fut: fn() -> BoxFut = core::mem::transmute(ASYNC_MAIN);
        // let main_fut = main_fut();
        // let main_task = new_task(main_fut, "main".to_string(), axconfig::TASK_STACK_SIZE);
        // main_task.init_executor(kexecutor.clone());
        // Executor::add_task(main_task);
        // executor::EXECUTORS.lock().insert(0, kexecutor.clone());
        let main_fut = Box::pin(async { main_fut() });
        let main_task = new_task(main_fut, "main".to_string(), axconfig::TASK_STACK_SIZE);
        main_task.init_executor(kexecutor.clone());
        Executor::add_task(main_task);
        executor::EXECUTORS.lock().insert(0, kexecutor.clone());
        CurrentExecutor::init_current(kexecutor);
    };
}

pub(crate) fn init_secondary() {
    assert!(KERNEL_EXECUTOR.is_init());
    let kexecutor = KERNEL_EXECUTOR.clone();
    unsafe { CurrentExecutor::init_current(kexecutor) };
}
