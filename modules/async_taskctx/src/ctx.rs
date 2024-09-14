use crate::arch::TaskContext;
use core::{
    future::Future, mem::MaybeUninit, panic, pin::Pin 
};
extern crate alloc;
use alloc::boxed::Box;
use memory_addr::VirtAddr;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
#[repr(usize)]
/// The policy of the scheduler
pub enum ContextType {
    /// The default coroutine context
    COROUTINE,
    /// The kernel thread context
    THREAD,
    /// Unknown context
    UNKNOWN,
}

/// The task context combined future and traditional context
pub struct Context {
    /// The task's thread context
    pub thread_ctx: TaskContext,
    /// The task's future context
    pub fut: MaybeUninit<Pin<Box<dyn Future<Output = i32> + 'static + Send>>>,
    /// The context type
    pub ctx_type: ContextType,
}

impl Context {
    /// Creates a new default context for a new task.
    pub const fn new() -> Self {
        Self {
            thread_ctx: TaskContext::new(),
            fut: MaybeUninit::uninit(),
            ctx_type: ContextType::COROUTINE,
        }
    }

    /// Initializes the context for a new task, with the given entry point and
    /// kernel stack.
    pub fn init(&mut self, entry: usize, kstack_top: VirtAddr, tls_area: VirtAddr) {
        self.thread_ctx.sp = kstack_top.as_usize();
        self.thread_ctx.ra = entry;
        self.thread_ctx.tp = tls_area.as_usize();
    }
    
    /// Initializes the context for a new task, with the given future.
    pub fn init_future<F, T>(&mut self, future: F)
    where
        F: FnOnce() -> T,
        T: Future<Output = i32> + 'static + Send,
    {
        self.fut.write(Box::pin(future()));
    }

    /// init_box_future
    pub fn init_box_future(&mut self, future: Pin<Box<dyn Future<Output = i32> + 'static + Send>>) {
        self.fut.write(future);
    }

    /// Set the context type
    pub fn set_ctx_type(&mut self, ctx_type: ContextType) {
        self.ctx_type = ctx_type;
    }

    /// Set the kstack top when a task is ready to run
    pub fn set_kstack_top(&mut self, kstack_top: VirtAddr) {
        self.thread_ctx.sp = kstack_top.as_usize();
    }

    pub fn thread_saved_fp(&self) -> usize {
        self.thread_ctx.s0 as usize
    }

    pub fn thread_saved_pc(&self) -> usize {
        self.thread_ctx.ra as usize
    }
}

/// Switches the context from the current task to the next task.
/// Poll future depends on the coroutine runtime(the task manager and scheduler),
/// so it will use a closure to poll the future.
/// 
/// 1. If the previous task and next task are both coroutines, just call the closure.
/// 2. If the previous task is a coroutine and the next task is a thread, 
///    directly restore the next task's context.
/// 3. If the previous task and next task are both threads, call the `context_switch` function.
/// 4. If the previous task is a thread and the next task is a coroutine, due to running coroutine is 
///    based on the hardware thread abstraction, it will switch to a new context, which is actually the 
///    function to run a coroutine.
/// 
/// # Safety
///
/// This function is unsafe because it directly manipulates the CPU registers.
pub unsafe extern "C" fn switch(prev_ctx: &mut Context, next_ctx: &mut Context, f: impl FnOnce()) {
    let prev_type = prev_ctx.ctx_type;
    let next_type = next_ctx.ctx_type;
    // No matter what the next_ctx is, we should set it to COROUTINE
    match (prev_type, next_type) {
        (ContextType::COROUTINE, ContextType::COROUTINE) => f(),
        (ContextType::COROUTINE, ContextType::THREAD) => crate::restore_context(&mut next_ctx.thread_ctx),
        (ContextType::THREAD, _) => {
            crate::context_switch(&mut prev_ctx.thread_ctx, &mut next_ctx.thread_ctx);
        },
        (_, _) => panic!("Unsupport context switch"),
    }
}