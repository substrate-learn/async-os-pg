


use alloc::{collections::btree_map::BTreeMap, sync::Arc};
use lazy_init::LazyInit;
use executor::*;
use spinlock::SpinNoIrqOnly;
use taskctx::Scheduler;

pub(crate) static KERNEL_EXECUTOR: LazyInit<Arc<Executor>> = LazyInit::new();
static EXECUTORS: SpinNoIrqOnly<BTreeMap<u64, ExecutorRef>> = SpinNoIrqOnly::new(BTreeMap::new());

// Initializes the trampoline (for the primary CPU).
pub fn init() {
    info!("Initialize trampoline...");
    taskctx::init();
    let kexecutor = Arc::new(Executor::new_init());
    KERNEL_EXECUTOR.init_by(kexecutor.clone());
    EXECUTORS.lock().insert(0, kexecutor.clone());
    unsafe { CurrentExecutor::init_current(kexecutor) };
    #[cfg(feature = "irq")]
    sync::init();
    info!("  use {} scheduler.", Scheduler::scheduler_name());
}

#[cfg(feature = "smp")]
/// Initializes the trampoline for secondary CPUs.
pub fn init_secondary() {
    assert!(KERNEL_EXECUTOR.is_init());
    taskctx::init();
    let kexecutor = KERNEL_EXECUTOR.clone();
    unsafe { CurrentExecutor::init_current(kexecutor) };
}
