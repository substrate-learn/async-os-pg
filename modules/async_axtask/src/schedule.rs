use crate::{current, current_executor, AxTaskRef};

#[cfg(feature = "future")]
use taskctx::ContextType;
#[cfg(all(feature = "future", feature = "monolithic"))]
use axhal::arch::TrapFrame;

/// 任务之间的等待关系，可以将任务实现为 子future，通过 future 之间的关系来实现
/// 并且通过专门的接口即可，不需要在这里单独定义这个关系
/// 这里的实现实际上是 future 任务之间的关系，这些任务也可以通过 join 等接口来实现

// /// A map to store tasks' wait queues, which stores tasks that are waiting for this task to exit.
// pub(crate) static WAIT_FOR_TASK_EXITS: SpinNoIrq<BTreeMap<u64, Arc<WaitQueue>>> =
//     SpinNoIrq::new(BTreeMap::new());

// pub(crate) fn add_wait_for_exit_queue(task: &AxTaskRef) {
//     WAIT_FOR_TASK_EXITS
//         .lock()
//         .insert(task.id().as_u64(), Arc::new(WaitQueue::new()));
// }

// pub(crate) fn get_wait_for_exit_queue(task: &AxTaskRef) -> Option<Arc<WaitQueue>> {
//     WAIT_FOR_TASK_EXITS.lock().get(&task.id().as_u64()).cloned()
// }

// /// When the task exits, notify all tasks that are waiting for this task to exit, and
// /// then remove the wait queue of the exited task.
// pub(crate) fn notify_wait_for_exit(task: &AxTaskRef) {
//     if let Some(wait_queue) = WAIT_FOR_TASK_EXITS.lock().remove(&task.id().as_u64()) {
//         wait_queue.notify_all();
//     }
// }

// pub(crate) fn exit_current(exit_code: i32) -> ! {
//     let curr = crate::current();
//     debug!("task exit: {}, exit_code={}", curr.id_name(), exit_code);
//     curr.set_state(TaskState::Exited);
//     // maybe others join on this thread
//     // must set state before notify wait_exit
//     notify_wait_for_exit(curr.as_task_ref());
//     current_executor().kick_exited_task(curr.as_task_ref());
//     if curr.is_init() {
//         Executor::clean_all();
//         axhal::misc::terminate();
//     } else {
//         curr.set_exit_code(exit_code);
//         schedule();
//     }
//     unreachable!("exit_current");
// }

// pub(crate) fn yield_current() {
//     let curr = crate::current();
//     assert!(curr.is_runable());
//     trace!("task yield: {}", curr.id_name());
//     #[cfg(feature = "future")]
//     curr.set_ctx_type(taskctx::ContextType::THREAD);
//     schedule();
// }

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
            crate::timers::set_alarm_wakeup(deadline, cx.waker().clone());
            core::task::Poll::Pending
        } else {
            // may wake up by others, cancel the alarm
            crate::timers::cancel_alarm(cx.waker());
            // return whether the deadline has passed
            core::task::Poll::Ready(axhal::time::current_time() >= deadline)
        }
    }).await
}

// #[cfg(feature = "irq")]
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

pub fn schedule() {
    unimplemented!()
}
