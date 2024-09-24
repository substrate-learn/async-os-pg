use crate::{current, current_executor, AxTaskRef};

#[cfg(feature = "future")]
use taskctx::ContextType;
#[cfg(all(feature = "future", feature = "monolithic"))]
use axhal::arch::TrapFrame;

/// 任务之间的等待关系，可以将任务实现为 子future，通过 future 之间的关系来实现
/// 并且通过专门的接口即可，不需要在这里单独定义这个关系
/// 这里的实现实际上是 future 任务之间的关系，这些任务也可以通过 join 等接口来实现

pub async fn schedule_timeout(deadline: axhal::time::TimeValue) -> bool {
    let curr = crate::current();
    debug!("task sleep: {}, deadline={:?}", curr.id_name(), deadline);
    assert!(!curr.is_idle());
    let mut flag = false;
    // Not directly use the cx.waker().wake_by_ref()
    // During the timer interrupt, cx.waker().wake_by_ref() will wake up the task
    core::future::poll_fn(|cx| {
        if !flag {
            flag = true;
            axsync::set_alarm_wakeup(deadline, cx.waker().clone());
            core::task::Poll::Pending
        } else {
            // may wake up by others, cancel the alarm
            axsync::cancel_alarm(cx.waker());
            // return whether the deadline has passed
            core::task::Poll::Ready(axhal::time::current_time() >= deadline)
        }
    }).await
}

#[cfg(feature = "irq")]
pub fn scheduler_timer_tick() {
    if let Some(curr) = crate::current_may_uninit() {
        if !curr.is_idle() && current_executor().task_tick(curr.as_task_ref()) {
            #[cfg(feature = "preempt")]
            curr.set_preempt_pending(true);
        }
    }    
}

pub fn set_current_priority(prio: isize) -> bool {
    current_executor().set_priority(current().as_task_ref(), prio)
}

pub fn wakeup_task(task: AxTaskRef) {
    log::debug!("wakeup task: {}", task.id_name());
    task.get_executor().put_prev_task(task, false);
}

#[cfg(feature = "preempt")]
pub fn preempt_schedule(tf: &axhal::arch::TrapFrame) {
    let curr = current();
    #[cfg(feature = "preempt")]
    curr.set_preempt_pending(false);
    curr.set_preempt_ctx(tf);
    let new_kstack_top = crate::current_stack_top();
    let ra = crate::run_idle as usize;
    crate::task::CurrentTask::clean_current();
    let waker = crate::waker::waker_from_task(curr.as_task_ref());
    waker.wake();
    unsafe { axhal::arch::jump(ra, new_kstack_top); }
}
