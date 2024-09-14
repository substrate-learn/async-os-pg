use core::{
    mem::ManuallyDrop, 
    ops::Deref,
};
use crate::{
    task::{CurrentTask, run_future}, AxTaskRef, Scheduler
};
use spinlock::{SpinNoIrq, SpinNoIrqOnly};
use alloc::{
    sync::Arc, 
    collections::BTreeMap,
};
use scheduler::BaseScheduler;

// id -> Executor(Process)
pub(super) static EXECUTORS: SpinNoIrqOnly<BTreeMap<u64, Arc<Executor>>> = SpinNoIrqOnly::new(BTreeMap::new());

pub struct Executor {
    /// Executor SCHEDULER
    scheduler: SpinNoIrq<Scheduler>,
    /// The stack pool of the executor
    stack_pool: SpinNoIrq<crate::stack_pool::StackPool>,
}


unsafe impl Sync for Executor {}
unsafe impl Send for Executor {}

impl Executor {
    pub fn new() -> Self {
        let mut scheduler = Scheduler::new();
        scheduler.init();
        Executor {
            scheduler: SpinNoIrq::new(scheduler),
            stack_pool: SpinNoIrq::new(crate::stack_pool::StackPool::new()),
        }
    }

    #[inline]
    /// Pick one task from Executor
    pub(crate) fn pick_next_task(&self) -> Option<AxTaskRef> {
        self.scheduler
            .lock()
            .pick_next_task()
    }

    #[inline]
    /// Add curr task to Executor, it ususally add to back
    pub(crate) fn put_prev_task(&self, task: AxTaskRef, front: bool) {
        self.scheduler.lock().put_prev_task(task, front);
    }

    #[inline]
    /// Add task to Executor, now just put it to own Executor
    /// TODO: support task migrate on differ Executor
    pub(crate) fn add_task(task: AxTaskRef) {
        task.get_executor().scheduler.lock().add_task(task);
    }

    #[inline]
    /// Executor Clean
    pub(crate) fn task_tick(&self, task: &AxTaskRef) -> bool {
        self.scheduler.lock().task_tick(task)
    }

    #[inline]
    /// Executor Clean
    pub(crate) fn set_priority(&self, task: &AxTaskRef, prio: isize) -> bool {
        self.scheduler.lock().set_priority(task, prio)
    }

    pub(crate) fn run(&self) -> ! {
        loop {
            if let Some(task) = self.pick_next_task() {
                unsafe { CurrentTask::init_current(task.clone()) };
                run_future(task);
            } else {
                #[cfg(feature = "irq")]
                axhal::arch::wait_for_irqs();
            }
        }
    }

}

impl Executor {    
    #[inline]
    /// Alloc a stack
    pub fn alloc_stack(&self) -> taskctx::TaskStack {
        self.stack_pool.lock().alloc()
    }

    #[inline]
    /// Recycle the stack
    pub fn recycle_stack(&self, stack: taskctx::TaskStack) {
        self.stack_pool.lock().recycle(stack)
    }
}

/// A wrapper of [`Arc<Executor>`] as the current executor.
pub struct CurrentExecutor(ManuallyDrop<Arc<Executor>>);

impl CurrentExecutor {
    pub(crate) fn try_get() -> Option<Self> {
        let ptr: *const Executor = super::current::current_executor_ptr();
        if !ptr.is_null() {
            Some(Self(unsafe { ManuallyDrop::new(Arc::from_raw(ptr)) }))
        } else {
            None
        }
    }

    pub(crate) fn get() -> Self {
        Self::try_get().expect("current executor is uninitialized")
    }

    /// Converts [`CurrentTask`] to [`Arc<Executor>`].
    pub fn as_ref(&self) -> &Arc<Executor> {
        &self.0
    }

    pub(crate) fn clone(&self) -> Arc<Executor> {
        self.0.deref().clone()
    }

    pub(crate) fn ptr_eq(&self, other: &Arc<Executor>) -> bool {
        Arc::ptr_eq(&self.0, other)
    }

    pub(crate) unsafe fn init_current(init_executor: Arc<Executor>) {
        let ptr = Arc::into_raw(init_executor);
        super::current::set_current_executor_ptr(ptr);
    }

    pub(crate) unsafe fn set_current(prev: Self, next: Arc<Executor>) {
        let Self(arc) = prev;
        ManuallyDrop::into_inner(arc); // `call Arc::drop()` to decrease prev task reference count.
        let ptr = Arc::into_raw(next);
        super::current::set_current_executor_ptr(ptr);
    }
}

impl Deref for CurrentExecutor {
    type Target = Arc<Executor>;
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}