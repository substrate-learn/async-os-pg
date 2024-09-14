use super::Executor;
use lazy_init::LazyInit;
use alloc::sync::Arc;

pub(crate) static KERNEL_EXECUTOR: LazyInit<Arc<Executor>> = LazyInit::new();
