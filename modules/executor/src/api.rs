use taskctx::CurrentTask;
use crate::{CurrentExecutor, TID2TASK};
use core::{future::Future, task::Poll};
use alloc::{boxed::Box, string::String, sync::Arc};
use taskctx::{BaseScheduler, Task, TaskInner, TaskRef, TaskState};

pub fn current_task_may_uninit() -> Option<CurrentTask> {
    CurrentTask::try_get()
}

pub fn current_task() -> CurrentTask {
    CurrentTask::get()
}

pub fn current_executor() -> CurrentExecutor {
    CurrentExecutor::get()
}

/// Spawns a new task with the given parameters.
/// 
/// Returns the task reference.
pub fn spawn_raw<F, T>(f: F, name: String) -> TaskRef
where
    F: FnOnce() -> T,
    T: Future<Output = i32> + 'static,
{
    let scheduler = current_executor().get_scheduler();
    let task = Arc::new(Task::new(
        TaskInner::new(name, scheduler.clone(), Box::pin(f()))
    ));
    scheduler.lock().add_task(task.clone());    
    task
}

pub async fn exit() {
    let curr = current_task();
    TID2TASK.lock().await.remove(&curr.id().as_u64());
}

/// Spawns a new task with the default parameters.
/// 
/// The default task name is an empty string. The default task stack size is
/// [`axconfig::TASK_STACK_SIZE`].
/// 
/// Returns the task reference.
pub fn spawn<F, T>(f: F) -> TaskRef
where
    F: FnOnce() -> T,
    T: Future<Output = i32> + 'static,
{
    spawn_raw(f, "".into())
}

/// Current task gives up the CPU time voluntarily, and switches to another
/// ready task.
pub fn yield_now() -> YieldFuture {
    YieldFuture::new()
}

pub struct YieldFuture(bool);

impl YieldFuture {
    pub fn new() -> Self {
        Self(false)
    }
}

impl Future for YieldFuture {
    type Output = ();
    fn poll(self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.get_mut().0 = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

/// Current task is going to sleep for the given duration.
/// 
/// If the feature `irq` is not enabled, it uses busy-wait instead.
pub fn sleep(dur: core::time::Duration) -> SleepFuture {
    SleepFuture::new(axhal::time::current_time() + dur)
}

#[derive(Debug)]
pub struct SleepFuture {
    #[cfg(feature = "irq")]
    has_sleep: bool,
    deadline: axhal::time::TimeValue,
}

impl SleepFuture {
    pub fn new(deadline: axhal::time::TimeValue) -> Self {
        Self {
            #[cfg(feature = "irq")]
            has_sleep: false,
            deadline,
        }
    }
}

impl Future for SleepFuture {
    type Output = bool;
    fn poll(self: core::pin::Pin<&mut Self>, _cx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
        let deadline = self.deadline;
        #[cfg(feature = "irq")]
        if !self.has_sleep {
            self.get_mut().has_sleep = true;
            sync::set_alarm_wakeup(deadline, _cx.waker().clone());
            Poll::Pending
        } else {
            sync::cancel_alarm(_cx.waker());
            Poll::Ready(axhal::time::current_time() >= deadline)
        }
        #[cfg(not(feature = "irq"))]
        {
            axhal::time::busy_wait_until(deadline);
            Poll::Ready(true)
        }
    }
}

/// Current task is going to sleep, it will be woken up at the given deadline.
///
/// If the feature `irq` is not enabled, it uses busy-wait instead.
pub fn sleep_until(deadline: axhal::time::TimeValue) -> SleepFuture{
    SleepFuture::new(deadline)
}

/// Current task is going to sleep, it will be woken up when the given task exits.
/// 
/// If the given task is already exited, it will return immediately.
/// If the 
pub fn join(task: &TaskRef) -> JoinFuture {
    JoinFuture::new(task.clone())
}

pub struct JoinFuture {
    task: TaskRef,
}

impl JoinFuture {
    pub fn new(task: TaskRef) -> Self {
        Self { task }
    }
}

impl Future for JoinFuture {
    type Output = Option<i32>;

    fn poll(self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.task.state() == TaskState::Exited {
            Poll::Ready(Some(this.task.get_exit_code()))
        } else {
            this.task.join(cx.waker().clone());
            Poll::Pending
        }
    }
}

/// Set the priority for current task.
///
/// The range of the priority is dependent on the underlying scheduler. For
/// example, in the [CFS] scheduler, the priority is the nice value, ranging from
/// -20 to 19.
///
/// Returns `true` if the priority is set successfully.
///
/// [CFS]: https://en.wikipedia.org/wiki/Completely_Fair_Scheduler
pub fn set_priority(prio: isize) -> bool {
    current_executor().set_priority(current_task().as_task_ref(), prio)
}
