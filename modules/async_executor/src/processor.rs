
use crate::{stack_pool::StackPool, Executor, executor::{EXECUTORS, KERNEL_EXECUTOR}};
use lazy_init::LazyInit;
use spinlock::SpinNoIrq;
use alloc::sync::Arc;
use taskctx::TaskStack;

#[percpu::def_percpu]
static PROCESSOR: LazyInit<Processor> = LazyInit::new();

pub struct Processor {
    executor: SpinNoIrq<Arc<Executor>>,
    stack_pool: SpinNoIrq<StackPool>,
}

impl Processor {
    fn new(executor: Arc<Executor>) -> Self {
        let processor = Self { 
            executor: SpinNoIrq::new(executor), 
            stack_pool: SpinNoIrq::new(StackPool::new())
        };
        processor.stack_pool.lock().init();
        processor
    }

    pub(crate) fn current_executor(&self) -> Arc<Executor> {
        self.executor.lock().clone()
    }
}

unsafe impl Sync for Processor {}
unsafe impl Send for Processor {}

pub fn current_processor() -> &'static Processor {
    unsafe { PROCESSOR.current_ref_raw() }
}

pub fn pick_current_stack() -> TaskStack {
    current_processor().stack_pool.lock().pick_current_stack()
}

pub fn current_stack_top() -> usize {
    current_processor().stack_pool.lock().current_stack().top().as_usize()
}

pub fn put_prev_stack(kstack: TaskStack) {
    current_processor().stack_pool.lock().put_prev_stack(kstack)
}

pub(crate) fn init() {
    let kexecutor = Arc::new(Executor::new_init());
    KERNEL_EXECUTOR.init_by(kexecutor.clone());
    EXECUTORS.lock().insert(0, kexecutor.clone());
    let processor = Processor::new(kexecutor);
    PROCESSOR.with_current(|i| i.init_by(processor));
}

pub(crate) fn init_secondary() {
    assert!(KERNEL_EXECUTOR.is_init());
    let kexecutor = KERNEL_EXECUTOR.clone();
    let processor = Processor::new(kexecutor);
    PROCESSOR.with_current(|i| i.init_by(processor));
}
