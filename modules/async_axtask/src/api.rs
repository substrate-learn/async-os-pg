use core::{future::Future, task::{Poll, Waker}};

use crate::{executor::{CurrentExecutor, Executor}, task::{new_task, CurrentTask, TaskState}, AxTaskRef, Scheduler};
use alloc::{string::String, boxed::Box};

/// Gets the current executor.
///
/// # Panics
///
/// Panics if the current task is not initialized.
pub fn current_executor() -> CurrentExecutor {
    CurrentExecutor::get()
}

/// Gets the current task, or returns [`None`] if the current task is not
/// initialized.
pub fn current_may_uninit() -> Option<CurrentTask> {
    CurrentTask::try_get()
}

/// Gets the current task.
///
/// # Panics
///
/// Panics if the current task is not initialized.
pub fn current() -> CurrentTask {
    CurrentTask::get()
}

pub fn clear_current() {
    CurrentTask::clean_current();
}

/// Initializes the task scheduler (for the primary CPU).
pub fn init_scheduler() {
    info!("Initialize scheduling...");
    crate::executor::init();
    crate::timers::init();
    info!("  use {} scheduler.", Scheduler::scheduler_name());
}

/// Initializes the task scheduler for secondary CPUs.
pub fn init_scheduler_secondary() {
    crate::executor::init_secondary();
}

/// Exits the current task.
pub fn exit(_exit_code: i32) -> ! {
    axhal::misc::terminate();
}

#[cfg(feature = "irq")]
#[doc(cfg(feature = "irq"))]
/// Handles periodic timer ticks for the task manager.
///
/// For example, advance scheduler states, checks timed events, etc.
pub fn on_timer_tick() {
    crate::timers::check_events();
    crate::schedule::scheduler_timer_tick();
}

#[cfg(feature = "preempt")]
/// Checks if the current task should be preempted.
/// This api called after handle irq,it may be on a
/// disable_preempt ctx
pub fn current_check_preempt_pending() {
    log::error!("current_check_preempt_pending");
    if let Some(curr) = current_may_uninit() {
        // if task is already exited or blocking,
        // no need preempt, they are rescheduling
        if curr.get_preempt_pending() && curr.can_preempt() && !curr.is_exited() && !curr.is_blocking()
        {
            debug!(
                "current {} is to be preempted , allow {}",
                curr.id_name(),
                curr.can_preempt()
            );
            crate::schedule::preempt_schedule()
        }
    }
}


#[no_mangle]
extern "C" fn main() {
    current_executor().run()
}

/// The idle task routine.
///
/// It runs an infinite loop that keeps calling [`yield_now()`].
#[no_mangle]
pub fn run_idle() -> ! {
    current_executor().run()
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
    has_sleep: bool,
    deadline: axhal::time::TimeValue,
}

impl SleepFuture {
    pub fn new(deadline: axhal::time::TimeValue) -> Self {
        Self {
            has_sleep: false,
            deadline,
        }
    }
}

impl Future for SleepFuture {
    type Output = bool;
    fn poll(self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
        let deadline = self.deadline;
        if !self.has_sleep {
            self.get_mut().has_sleep = true;
            crate::timers::set_alarm_wakeup(deadline, cx.waker().clone());
            Poll::Pending
        } else {
            crate::timers::cancel_alarm(cx.waker());
            Poll::Ready(axhal::time::current_time() >= self.deadline)
        }
    }
}

pub fn current_waker() -> Waker {
    crate::waker::waker_from_task(current().as_task_ref())
}

/// Current task is going to sleep, it will be woken up at the given deadline.
///
/// If the feature `irq` is not enabled, it uses busy-wait instead.
pub fn sleep_until(deadline: axhal::time::TimeValue) -> SleepFuture{
    SleepFuture::new(deadline)
}

/// wake up task
pub fn wakeup_task(task: AxTaskRef) {
    crate::schedule::wakeup_task(task)
}

/// Spawns a new task with the given parameters.
///
/// Returns the task reference.
pub fn spawn_raw<F, T>(f: F, name: String, stack_size: usize) -> AxTaskRef
where
    F: FnOnce() -> T,
    T: Future<Output = i32> + 'static + Send,
{
    let task = new_task(
        Box::pin(f()),
        name,
        stack_size,
        #[cfg(feature = "monolithic")]
        KERNEL_PROCESS_ID,
        #[cfg(feature = "monolithic")]
        0,
    );
    let current_executor = current_executor();
    task.init_executor(current_executor.clone());
    Executor::add_task(task.clone());
    task
}

/// Spawns a new task with the default parameters.
///
/// The default task name is an empty string. The default task stack size is
/// [`axconfig::TASK_STACK_SIZE`].
///
/// Returns the task reference.
pub fn spawn<F, T>(f: F) -> AxTaskRef
where
    F: FnOnce() -> T,
    T: Future<Output = i32> + 'static + Send,
{
    spawn_raw(f, "".into(), axconfig::TASK_STACK_SIZE)
}

/// Current task is going to sleep, it will be woken up when the given task exits.
///
/// If the given task is already exited, it will return immediately.
/// If the 
pub fn join(task: &AxTaskRef) -> JoinFuture {
    JoinFuture::new(task.clone())
}

pub struct JoinFuture {
    task: AxTaskRef,
}

impl JoinFuture {
    pub fn new(task: AxTaskRef) -> Self {
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
    crate::schedule::set_current_priority(prio)
}

pub fn dump_curr_backtrace() {
    dump_task_backtrace(current().as_task_ref().clone());
}

pub fn dump_task_backtrace(task: AxTaskRef) {
    use axbacktrace::{dump_backtrace, Unwind, UnwindIf, StackInfo};

    let stack_low = task.get_kernel_stack_down().unwrap();
    let stack_high = task.get_kernel_stack_top().unwrap();
    info!("dump task: {}, stack range: {:#016x}: {:#016x}", 
        task.id_name(), stack_low, stack_high);
    let stack_info = StackInfo::new(stack_low,stack_high);

    //Init Unwind instance from current context
    let curr = crate::current();
    let mut unwind = if curr.ptr_eq(&task) {
        Unwind::new_from_cur_ctx(stack_info)
    } else {
        let (pc, fp) = task.ctx_unwind();
        Unwind::new(pc,fp,stack_info)
    };
    // dump current task trace
    dump_backtrace(&mut unwind);
}
