
use crate::{
    task::run_future, AxTaskRef, Scheduler
};
use spinlock::{SpinNoIrq, SpinNoIrqOnly};
use alloc::{
    sync::Arc, 
    collections::BTreeMap,
};
use scheduler::BaseScheduler;
use lazy_init::LazyInit;

// use axmem::MemorySet;

pub(crate) static KERNEL_EXECUTOR: LazyInit<Arc<Executor>> = LazyInit::new();

// id -> Executor(Process)
pub(super) static EXECUTORS: SpinNoIrqOnly<BTreeMap<u64, Arc<Executor>>> = SpinNoIrqOnly::new(BTreeMap::new());

pub struct Executor {
    /// Executor SCHEDULER
    scheduler: SpinNoIrq<Scheduler>,

    // #[cfg(feature = "monolithic")]
    // pub memory_set: MemorySet

}


unsafe impl Sync for Executor {}
unsafe impl Send for Executor {}

impl Executor {
    pub fn new() -> Self {
        let mut scheduler = Scheduler::new();
        scheduler.init();
        Executor {
            scheduler: SpinNoIrq::new(scheduler),
            // memory_set: MemorySet::new_memory_set()
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
                run_future(task);
            } else {
                #[cfg(feature = "irq")]
                axhal::arch::wait_for_irqs();
            }
        }
    }

}
