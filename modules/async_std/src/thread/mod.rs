//! Native threads.

#[cfg(feature = "multitask")]
mod multi;
#[cfg(feature = "multitask")]
pub use multi::*;

use async_api::task as api;

/// Current thread gives up the CPU time voluntarily, and switches to another
/// ready thread.
///
/// For single-threaded configuration (`multitask` feature is disabled), we just
/// relax the CPU and wait for incoming interrupts.
pub async fn yield_now() {
    api::ax_yield_now().await;
}

// /// Exits the current thread.
// ///
// /// For single-threaded configuration (`multitask` feature is disabled),
// /// it directly terminates the main thread and shutdown.
// pub fn exit(exit_code: i32) -> ! {
//     api::ax_exit(exit_code);
// }

/// Current thread is going to sleep for the given duration.
///
/// If one of `multitask` or `irq` features is not enabled, it uses busy-wait
/// instead.
pub async fn sleep(dur: core::time::Duration) {
    sleep_until(async_api::time::ax_current_time() + dur).await;
}

/// Current thread is going to sleep, it will be woken up at the given deadline.
///
/// If one of `multitask` or `irq` features is not enabled, it uses busy-wait
/// instead.
pub async fn sleep_until(deadline: async_api::time::AxTimeValue) {
    api::ax_sleep_until(deadline).await;
}
