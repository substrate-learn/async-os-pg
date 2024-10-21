use async_axhal::{mem::VirtAddr, paging::MappingFlags};
use executor::{current_executor, current_task_may_uninit};
use taskctx::{TrapFrame, TrapStatus};


#[cfg(feature = "irq")]
#[doc(cfg(feature = "irq"))]
/// Handles periodic timer ticks for the task manager.
///
/// For example, advance scheduler states, checks timed events, etc.
pub fn on_timer_tick() {
    use executor::CurrentExecutor;
    sync::check_events();
    // warn!("on_timer_tick");
    if let Some(curr) = current_task_may_uninit() {
        if CurrentExecutor::get().task_tick(curr.as_task_ref()) {
            #[cfg(feature = "preempt")]
            curr.set_preempt_pending(true);
        }
    }    
}

/// To deal with the page fault
pub async fn handle_page_fault(addr: VirtAddr, flags: MappingFlags) {
    let current_executor = current_executor();
    if current_executor
        .memory_set
        .lock().await.
        handle_page_fault(addr, flags).await
        .is_ok() {
        async_axhal::arch::flush_tlb(None);
    }
}

pub fn handle_irq(_irq_num: usize, tf: &mut TrapFrame) {
    #[cfg(feature = "irq")]
    {
        let guard = kernel_guard::NoPreempt::new();
        async_axhal::irq::dispatch_irq(_irq_num);
        drop(guard); // rescheduling may occur when preemption is re-enabled.
        tf.trap_status = TrapStatus::Done;

        #[cfg(feature = "preempt")]
        crate::current_check_preempt_pending(tf);
    }
}

pub async fn handle_user_irq(_irq_num: usize, tf: &mut TrapFrame) {
    #[cfg(feature = "irq")]
    {
        let guard = kernel_guard::NoPreempt::new();
        async_axhal::irq::dispatch_irq(_irq_num);
        drop(guard); // rescheduling may occur when preemption is re-enabled.

        tf.trap_status = TrapStatus::Done;
        #[cfg(feature = "preempt")]
        crate::current_check_user_preempt_pending(tf).await;
    }
}