use crate::{stat::TimeStat, Scheduler, TrapFrame};
use core::{cell::UnsafeCell, fmt, future::Future, pin::Pin, sync::atomic::{AtomicI32, AtomicU64, Ordering}, task::Waker};
use spinlock::SpinNoIrq;
use alloc::{boxed::Box, collections::vec_deque::VecDeque, string::String, sync::Arc};
#[cfg(feature = "preempt")]
use {
    core::sync::atomic::{AtomicUsize, AtomicBool},
    crate::TaskStack,
    spinlock::SpinNoIrqGuard
};

/// A unique identifier for a thread.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TaskId(u64);

static ID_COUNTER: AtomicU64 = AtomicU64::new(1);
impl TaskId {
    /// Create a new task ID.
    pub fn new() -> Self {
        Self(ID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Convert the task ID to a `u64`.
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}


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

pub struct TaskInner {
    fut: UnsafeCell<Pin<Box<dyn Future<Output = i32> + 'static>>>,
    utrap_frame: UnsafeCell<Option<TrapFrame>>,

    // executor: SpinNoIrq<Arc<Executor>>,
    pub(crate) wait_wakers: UnsafeCell<VecDeque<Waker>>,
    pub(crate) scheduler: SpinNoIrq<Arc<SpinNoIrq<Scheduler>>>,
    
    pub(crate) id: TaskId,
    pub(crate) name: UnsafeCell<String>,
    /// Whether the task is the initial task
    /// 
    /// If the task is the initial task, the kernel will terminate
    /// when the task exits.
    pub(crate) is_init: bool,
    pub(crate) state: SpinNoIrq<TaskState>,
    time: UnsafeCell<TimeStat>,
    exit_code: AtomicI32,
    clear_child_tid: AtomicU64,
    #[cfg(feature = "preempt")]
    /// Whether the task needs to be rescheduled
    ///
    /// When the time slice is exhausted, it needs to be rescheduled
    need_resched: AtomicBool,
    #[cfg(feature = "preempt")]
    /// The disable count of preemption
    ///
    /// When the task get a lock which need to disable preemption, it
    /// will increase the count. When the lock is released, it will
    /// decrease the count.
    ///
    /// Only when the count is zero, the task can be preempted.
    preempt_disable_count: AtomicUsize,
    #[cfg(feature = "preempt")]
    /// 在内核中发生抢占时的抢占上下文
    preempt_ctx: SpinNoIrq<Option<PreemptCtx>>,
}

#[cfg(feature = "preempt")]
pub struct PreemptCtx {
    pub kstack: TaskStack,
    pub trap_frame: *const TrapFrame,
}

#[cfg(feature = "preempt")]
impl PreemptCtx {
    pub(crate) fn new(kstack: TaskStack, trap_frame: *const TrapFrame) -> Self {
        let kstack_top = kstack.top().as_usize();
        let kstack_bottom = kstack.down().as_usize();
        assert!((trap_frame as usize) >= kstack_bottom);
        assert!((trap_frame as usize) < kstack_top, "{:#X} - [{:#X}, {:#X})", (trap_frame as usize), kstack_bottom, kstack_top);
        Self {
            kstack,
            trap_frame,
        }
    }
}

unsafe impl Send for TaskInner {}
unsafe impl Sync for TaskInner {}

impl TaskInner {

    pub fn new(
        name: String,
        scheduler: Arc<SpinNoIrq<Scheduler>>,
        fut: Pin<Box<dyn Future<Output = i32> + 'static>>,
    ) -> Self {
        let is_init = &name == "main";
        let t = Self {
            id: TaskId::new(),
            name: UnsafeCell::new(name),
            is_init,
            exit_code: AtomicI32::new(0),
            fut: UnsafeCell::new(fut),
            utrap_frame: UnsafeCell::new(None),
            wait_wakers: UnsafeCell::new(VecDeque::new()),
            scheduler: SpinNoIrq::new(scheduler),
            state: SpinNoIrq::new(TaskState::Runable),
            time: UnsafeCell::new(TimeStat::new()),
            clear_child_tid: AtomicU64::new(0),
            #[cfg(feature = "preempt")]
            need_resched: AtomicBool::new(false),
            #[cfg(feature = "preempt")]
            preempt_disable_count: AtomicUsize::new(0),
            #[cfg(feature = "preempt")]
            preempt_ctx: SpinNoIrq::new(None),
        };
        t
    }

    pub fn new_user(
        name: String,
        scheduler: Arc<SpinNoIrq<Scheduler>>,
        fut: Pin<Box<dyn Future<Output = i32> + 'static>>,
        utrap_frame: TrapFrame
        // task_type: TaskType,
    ) -> Self {
        let is_init = &name == "main";
        let t = Self {
            id: TaskId::new(),
            name: UnsafeCell::new(name),
            is_init,
            exit_code: AtomicI32::new(0),
            fut: UnsafeCell::new(fut),
            utrap_frame: UnsafeCell::new(Some(utrap_frame)),
            wait_wakers: UnsafeCell::new(VecDeque::new()),
            scheduler: SpinNoIrq::new(scheduler),
            state: SpinNoIrq::new(TaskState::Runable),
            time: UnsafeCell::new(TimeStat::new()),
            clear_child_tid: AtomicU64::new(0),
            #[cfg(feature = "preempt")]
            need_resched: AtomicBool::new(false),
            #[cfg(feature = "preempt")]
            preempt_disable_count: AtomicUsize::new(0),
            #[cfg(feature = "preempt")]
            preempt_ctx: SpinNoIrq::new(None),
        };
        t
    }

    /// 获取到任务的 Future
    pub fn get_fut(&self) -> &mut Pin<Box<dyn Future<Output = i32> + 'static>> {
        unsafe { &mut *self.fut.get() }
    }

    /// Gets the ID of the task.
    pub const fn id(&self) -> TaskId {
        self.id
    }

    /// Gets the name of the task.
    pub fn name(&self) -> &str {
        unsafe { (*self.name.get()).as_str() }
    }

    /// Sets the name of the task.
    pub fn set_name(&self, name: &str) {
        unsafe {
            *self.name.get() = String::from(name);
        }
    }

    /// Get a combined string of the task ID and name.
    pub fn id_name(&self) -> alloc::string::String {
        alloc::format!("Task({}, {:?})", self.id.as_u64(), self.name())
    }

    /// Whether the task has been inited
    #[inline]
    pub const fn is_init(&self) -> bool {
        self.is_init
    }

    /// Get the exit code
    #[inline]
    pub fn get_exit_code(&self) -> i32 {
        self.exit_code.load(Ordering::Acquire)
    }

    /// Set the task exit code
    #[inline]
    pub fn set_exit_code(&self, code: i32) {
        self.exit_code.store(code, Ordering::Release)
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

    pub fn get_scheduler(&self) -> Arc<SpinNoIrq<Scheduler>> {
        self.scheduler.lock().clone()
    }

    /// clear (zero) the child thread ID at the location pointed to by child_tid in clone args
    pub fn set_clear_child_tid(&self, tid: usize) {
        self.clear_child_tid.store(tid as u64, Ordering::Release)
    }

    /// get the pointer to the child thread ID
    pub fn get_clear_child_tid(&self) -> usize {
        self.clear_child_tid.load(Ordering::Acquire) as usize
    }

}

/// Methods for task switch
impl TaskInner {
    pub fn notify_waker_for_exit(&self) {
        let wait_wakers = unsafe { &mut *self.wait_wakers.get() };
        while let Some(waker) = wait_wakers.pop_front() {
            waker.wake();
        }
    }

    pub fn join(&self, waker: Waker) {
        let wait_wakers = unsafe { &mut *self.wait_wakers.get() };
        wait_wakers.push_back(waker);
    }

    pub fn utrap_frame(&self) -> Option<&mut TrapFrame> {
        unsafe {&mut *self.utrap_frame.get() }.as_mut()
    }
}

/// Methods for time statistics
impl TaskInner {
    #[inline]
    /// update the time information when the task is switched from user mode to kernel mode
    pub fn time_stat_from_user_to_kernel(&self, current_tick: usize) {
        let time = self.time.get();
        unsafe {
            (*time).switch_into_kernel_mode(self.id.as_u64() as isize, current_tick);
        }
    }

    #[inline]
    /// update the time information when the task is switched from kernel mode to user mode
    pub fn time_stat_from_kernel_to_user(&self, current_tick: usize) {
        let time = self.time.get();
        unsafe {
            (*time).switch_into_user_mode(self.id.as_u64() as isize, current_tick);
        }
    }

    #[inline]
    /// update the time information when the task is switched out
    pub fn time_stat_when_switch_from(&self, current_tick: usize) {
        let time = self.time.get();
        unsafe {
            (*time).swtich_from_old_task(self.id.as_u64() as isize, current_tick);
        }
    }

    #[inline]
    /// update the time information when the task is ready to be switched in
    pub fn time_stat_when_switch_to(&self, current_tick: usize) {
        let time = self.time.get();
        unsafe {
            (*time).switch_to_new_task(self.id.as_u64() as isize, current_tick);
        }
    }

    #[inline]
    /// output the time statistics
    ///
    /// The format is (user time, kernel time) in nanoseconds
    pub fn time_stat_output(&self) -> (usize, usize) {
        let time = self.time.get();
        unsafe { (*time).output() }
    }

    #[inline]
    /// 输出计时器信息
    /// (计时器周期，当前计时器剩余时间)
    /// 单位为us
    pub fn timer_output(&self) -> (usize, usize) {
        let time = self.time.get();
        unsafe { (*time).output_timer_as_us() }
    }

    #[inline]
    /// 设置计时器信息
    ///
    /// 若type不为None则返回成功
    pub fn set_timer(
        &self,
        timer_interval_ns: usize,
        timer_remained_ns: usize,
        timer_type: usize,
    ) -> bool {
        let time = self.time.get();
        unsafe { (*time).set_timer(timer_interval_ns, timer_remained_ns, timer_type) }
    }

    #[inline]
    /// 重置统计时间
    pub fn time_stat_reset(&self, current_tick: usize) {
        let time = self.time.get();
        unsafe {
            (*time).reset(current_tick);
        }
    }
}

#[cfg(feature = "preempt")]
impl TaskInner {
    /// Set the task waiting for reschedule
    #[inline]
    pub fn set_preempt_pending(&self, pending: bool) {
        self.need_resched.store(pending, Ordering::Release)
    }

    /// Get whether the task is waiting for reschedule
    #[inline]
    pub fn get_preempt_pending(&self) -> bool {
        self.need_resched.load(Ordering::Acquire)
    }

    /// Whether the task can be preempted
    #[inline]
    pub fn can_preempt(&self) -> bool {
        self.preempt_disable_count.load(Ordering::Acquire) == 0
    }

    /// Disable the preemption
    #[inline]
    pub fn disable_preempt(&self) {
        self.preempt_disable_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Enable the preemption by increasing the disable count
    ///
    /// Only when the count is zero, the task can be preempted
    #[inline]
    pub fn enable_preempt(&self) {
        self.preempt_disable_count.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get the number of preempt disable counter
    #[inline]
    pub fn preempt_num(&self) -> usize {
        self.preempt_disable_count.load(Ordering::Acquire)
    }

    pub fn set_preempt_ctx(&self, tf: &TrapFrame) {
        let mut sp: usize;
        unsafe { core::arch::asm!("mv {}, sp", out(reg) sp); }
        log::warn!("current sp: {:#x?}", sp);
        let preempt_ctx = PreemptCtx::new(crate::pick_current_stack(), tf);
        assert!(self.preempt_ctx.lock().is_none());
        self.preempt_ctx.lock().replace(preempt_ctx);
    }

    pub fn preempt_ctx_lock(&self) -> SpinNoIrqGuard<'_, Option<PreemptCtx>> {
        self.preempt_ctx.lock()
    }
}

impl fmt::Debug for TaskInner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TaskInner")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

impl Drop for TaskInner {
    fn drop(&mut self) {
        log::debug!("task drop: {}", self.id_name());
    }
}
