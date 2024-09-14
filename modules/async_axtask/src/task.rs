use core::{
    future::Future, mem::ManuallyDrop, ops::Deref, pin::Pin, task::Waker
};
use alloc::{sync::Arc, string::String, boxed::Box, collections::VecDeque};
use taskctx::TaskInner;
use crate::{executor::Executor, AxTask, AxTaskRef};
use spinlock::{SpinNoIrq, SpinNoIrqOnly};

/// The possible states of a task.
#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[allow(missing_docs)]
pub enum TaskState {
    Runable = 1,
    Blocking = 2,
    Blocked = 3,
    Exited = 4,
}

pub struct ScheduleTask {
    inner: TaskInner,
    /// Task state
    state: SpinNoIrqOnly<TaskState>,
    /// Task belong to which Executor
    executor: SpinNoIrq<Option<Arc<Executor>>>,
    ///
    wait_wakers: SpinNoIrq<VecDeque<Waker>>,
}

unsafe impl Send for ScheduleTask {}
unsafe impl Sync for ScheduleTask {}

impl ScheduleTask {
    pub fn new(inner: TaskInner) -> Self {
        Self {
            inner,
            state: SpinNoIrqOnly::new(TaskState::Runable),
            executor: SpinNoIrq::new(None),
            wait_wakers: SpinNoIrq::new(VecDeque::new()),
        }
    }

    #[inline]
    /// set the state of the task
    pub fn state(&self) -> TaskState {
        *self.state.lock()
    }

    #[inline]
    /// set the state of the task
    pub fn set_state(&self, state: TaskState) {
        *self.state.lock() = state
    }

    /// Whether the task is Exited
    #[inline]
    pub fn is_exited(&self) -> bool {
        matches!(*self.state.lock(), TaskState::Exited)
    }

    /// Whether the task is runnalbe
    #[inline]
    pub fn is_runable(&self) -> bool {
        matches!(*self.state.lock(), TaskState::Runable)
    }

    /// Whether the task is blocking
    #[inline]
    pub fn is_blocking(&self) -> bool {
        matches!(*self.state.lock(), TaskState::Blocking)
    }

    /// Whether the task is blocked
    #[inline]
    pub fn is_blocked(&self) -> bool {
        matches!(*self.state.lock(), TaskState::Blocked)
    }

    /// Init the executor
    #[inline]
    pub(crate) fn init_executor(&self, executor: Arc<Executor>) {
        *self.executor.lock() = Some(executor);
    }

    /// Get the executor
    #[inline]
    pub(crate) fn get_executor(&self) -> Arc<Executor> {
        self.executor
            .lock()
            .as_ref()
            .expect("task {} executor not init")
            .clone()
    }

    pub fn join(&self, waker: Waker) {
        self.wait_wakers.lock().push_back(waker);
    }

    pub fn notify_waker_for_exit(&self) {
        while let Some(waker) = self.wait_wakers.lock().pop_front() {
            waker.wake();
        }
    }
}

impl Deref for ScheduleTask {
    type Target = TaskInner;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub fn new_task(
    fut: Pin<Box<dyn Future<Output = i32> + 'static + Send>>,
    name: String, 
    stack_size: usize, 
) -> AxTaskRef {
    let inner = TaskInner::new_future(
        fut, 
        name, 
        stack_size
    );
    let task = Arc::new(AxTask::new(ScheduleTask::new(inner)));
    task
}

/// A wrapper of [`AxTaskRef`] as the current task.
pub struct CurrentTask(ManuallyDrop<AxTaskRef>);

impl CurrentTask {
    pub(crate) fn try_get() -> Option<Self> {
        let ptr: *const super::AxTask = taskctx::current_task_ptr();
        if !ptr.is_null() {
            Some(Self(unsafe { ManuallyDrop::new(AxTaskRef::from_raw(ptr)) }))
        } else {
            None
        }
    }

    pub(crate) fn get() -> Self {
        Self::try_get().expect("current task is uninitialized")
    }

    /// Converts [`CurrentTask`] to [`AxTaskRef`].
    pub fn as_task_ref(&self) -> &AxTaskRef {
        &self.0
    }

    pub(crate) fn clone(&self) -> AxTaskRef {
        self.0.deref().clone()
    }

    pub(crate) fn ptr_eq(&self, other: &AxTaskRef) -> bool {
        Arc::ptr_eq(&self.0, other)
    }

    pub(crate) unsafe fn init_current(init_task: AxTaskRef) {
        #[cfg(feature = "tls")]
        axhal::arch::write_thread_pointer(init_task.get_tls_ptr());
        let ptr = Arc::into_raw(init_task);
        taskctx::set_current_task_ptr(ptr);
    }

    pub(crate) fn clean_current() {
        let curr = Self::get();
        let Self(arc) = curr;
        ManuallyDrop::into_inner(arc); // `call Arc::drop()` to decrease prev task reference count.
        unsafe { taskctx::set_current_task_ptr(0 as *const AxTask) };
    }
}

impl Deref for CurrentTask {
    type Target = ScheduleTask;
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

pub(crate) fn run_future(task: AxTaskRef) {
    use core::task::{Context, Poll};
    let waker = crate::waker::waker_from_task(task.clone());
    unsafe {
        let ctx = &mut *task.ctx_mut_ptr();
        let fut = &mut (*ctx.fut.as_mut_ptr());
        if let Poll::Ready(exit_code) = fut.as_mut().poll(&mut Context::from_waker(&waker)) {
            task.set_ctx_type(taskctx::ContextType::COROUTINE);
            debug!("task exit: {}, exit_code={}", task.id_name(), exit_code);
            task.set_state(TaskState::Exited);
            task.set_exit_code(exit_code);
            task.notify_waker_for_exit();
            CurrentTask::clean_current();
            drop(waker);
            if task.is_init() {
                assert!(Arc::strong_count(&task) == 1, "count {}", Arc::strong_count(&task));
                drop(task);
                axhal::misc::terminate();
            }
        }
        // If the future is pending, its waker must be hold by other struts.
    }
}