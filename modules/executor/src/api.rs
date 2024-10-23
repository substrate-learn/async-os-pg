use taskctx::CurrentTask;
use crate::{flags::WaitStatus, CurrentExecutor, KERNEL_EXECUTOR_ID, TID2TASK};
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
        TaskInner::new(name, KERNEL_EXECUTOR_ID, scheduler.clone(), Box::pin(f()))
    ));
    scheduler.lock().add_task(task.clone());    
    task
}

pub async fn exit(exit_code: i32) {
    let curr = current_task();
    TID2TASK.lock().await.remove(&curr.id().as_u64());
    curr.set_exit_code(exit_code);
    curr.set_state(TaskState::Exited);
    let current_executor = current_executor();
    current_executor.set_exit_code(exit_code);
    current_executor.set_zombie(true);
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


/// 在当前进程找对应的子进程，并等待子进程结束
/// 若找到了则返回对应的pid
/// 否则返回一个状态
///
/// # Safety
///
/// 保证传入的 ptr 是有效的
pub async unsafe fn wait_pid(pid: i32, exit_code_ptr: *mut i32) -> Result<u64, WaitStatus> {
    // 获取当前进程
    let curr_process = current_executor();
    let mut exit_task_id: usize = 0;
    let mut answer_id: u64 = 0;
    let mut answer_status = WaitStatus::NotExist;
    for (index, child) in curr_process.children.lock().await.iter().enumerate() {
        if pid <= 0 {
            if pid == 0 {
                axlog::warn!("Don't support for process group.");
            }
            // 任意一个进程结束都可以的
            answer_status = WaitStatus::Running;
            if let Some(exit_code) = child.get_code_if_exit() {
                answer_status = WaitStatus::Exited;
                info!("wait pid _{}_ with code _{}_", child.pid().as_u64(), exit_code);
                exit_task_id = index;
                if !exit_code_ptr.is_null() {
                    unsafe {
                        // 因为没有切换页表，所以可以直接填写
                        *exit_code_ptr = exit_code << 8;
                    }
                }
                answer_id = child.pid().as_u64();
                break;
            }
        } else if child.pid().as_u64() == pid as u64 {
            // 找到了对应的进程
            if let Some(exit_code) = child.get_code_if_exit() {
                answer_status = WaitStatus::Exited;
                info!("wait pid _{}_ with code _{:?}_", child.pid().as_u64(), exit_code);
                exit_task_id = index;
                if !exit_code_ptr.is_null() {
                    unsafe {
                        *exit_code_ptr = exit_code << 8;
                        // 用于WEXITSTATUS设置编码
                    }
                }
                answer_id = child.pid().as_u64();
            } else {
                answer_status = WaitStatus::Running;
            }
            break;
        }
    }
    // 若进程成功结束，需要将其从父进程的children中删除
    if answer_status == WaitStatus::Exited {
        curr_process.children.lock().await.remove(exit_task_id);
        return Ok(answer_id);
    }
    Err(answer_status)
}