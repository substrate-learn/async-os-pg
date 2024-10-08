
use spinlock::{SpinNoIrq, SpinNoIrqOnly};
use alloc::{
    sync::Arc, 
    collections::BTreeMap,
};
use lazy_init::LazyInit;

use axmem::MemorySet;

pub(crate) static KERNEL_EXECUTOR: LazyInit<Arc<Executor>> = LazyInit::new();

// id -> Executor(Process)
pub(super) static EXECUTORS: SpinNoIrqOnly<BTreeMap<u64, Arc<Executor>>> = SpinNoIrqOnly::new(BTreeMap::new());

pub struct Executor {
    // #[cfg(feature = "monolithic")]
    pub memory_set: MemorySet
    
}


unsafe impl Sync for Executor {}
unsafe impl Send for Executor {}

impl Executor {
    pub fn new() -> Self {
        Executor {
            memory_set: MemorySet::new_memory_set()
        }
    }


}
