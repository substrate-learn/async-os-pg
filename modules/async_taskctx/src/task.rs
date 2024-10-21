
use crate::TimeStat;
use alloc::string::String;

use core::{
    cell::UnsafeCell,
    fmt,
    sync::atomic::{AtomicI32, AtomicU64, Ordering},
};
#[cfg(feature = "preempt")]
use core::sync::atomic::{AtomicBool, AtomicUsize};
use alloc::boxed::Box;
use core::{pin::Pin, future::Future};

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

#[derive(PartialEq, Eq, Clone, Copy)]
#[allow(non_camel_case_types)]
/// The policy of the scheduler
pub enum SchedPolicy {
    /// The default time-sharing scheduler
    SCHED_OTHER = 0,
    /// The first-in, first-out scheduler
    SCHED_FIFO = 1,
    /// The round-robin scheduler
    SCHED_RR = 2,
    /// The batch scheduler
    SCHED_BATCH = 3,
    /// The idle task scheduler
    SCHED_IDLE = 5,
    /// Unknown scheduler
    SCHED_UNKNOWN,
}

impl From<usize> for SchedPolicy {
    #[inline]
    fn from(policy: usize) -> Self {
        match policy {
            0 => SchedPolicy::SCHED_OTHER,
            1 => SchedPolicy::SCHED_FIFO,
            2 => SchedPolicy::SCHED_RR,
            3 => SchedPolicy::SCHED_BATCH,
            5 => SchedPolicy::SCHED_IDLE,
            _ => SchedPolicy::SCHED_UNKNOWN,
        }
    }
}

impl From<SchedPolicy> for isize {
    #[inline]
    fn from(policy: SchedPolicy) -> Self {
        match policy {
            SchedPolicy::SCHED_OTHER => 0,
            SchedPolicy::SCHED_FIFO => 1,
            SchedPolicy::SCHED_RR => 2,
            SchedPolicy::SCHED_BATCH => 3,
            SchedPolicy::SCHED_IDLE => 5,
            SchedPolicy::SCHED_UNKNOWN => -1,
        }
    }
}

#[derive(Clone, Copy)]
/// The status of the scheduler
pub struct SchedStatus {
    /// The policy of the scheduler
    pub policy: SchedPolicy,
    /// The priority of the scheduler policy
    pub priority: usize,
}

/// The inner task structure used as the minimal unit of scheduling.
pub struct TaskInner {
    id: TaskId,

    name: UnsafeCell<String>,

    /// Whether the task is the idle task
    is_idle: bool,
    /// Whether the task is the initial task
    ///
    /// If the task is the initial task, the kernel will terminate
    /// when the task exits.
    is_init: bool,

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

    set_child_tid: AtomicU64,
    clear_child_tid: AtomicU64,

    exit_code: AtomicI32,

    /// 时间统计, 无论是否为宏内核架构都可能被使用到
    #[allow(unused)]
    time: UnsafeCell<TimeStat>,

    fut: UnsafeCell<Pin<Box<dyn Future<Output = i32> + 'static>>>

}

unsafe impl Send for TaskInner {}
unsafe impl Sync for TaskInner {}

impl TaskInner {
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

    pub fn set_child_tid(&self, tid: usize) {
        self.set_child_tid.store(tid as u64, Ordering::Release)
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


impl TaskInner {
    pub fn new(
        name: String,
        fut: Pin<Box<dyn Future<Output = i32> + 'static>>,
    ) -> Self {
        let is_idle = &name == "idle";
        let is_init = &name == "main";
        let t = Self {
            id: TaskId::new(),
            name: UnsafeCell::new(name),
            is_idle,
            is_init,
            #[cfg(feature = "preempt")]
            need_resched: AtomicBool::new(false),
            #[cfg(feature = "preempt")]
            preempt_disable_count: AtomicUsize::new(0),
            set_child_tid: AtomicU64::new(0),
            clear_child_tid: AtomicU64::new(0),
            exit_code: AtomicI32::new(0),
            time: UnsafeCell::new(TimeStat::new()),
            fut: UnsafeCell::new(fut),
        };
        t
    }

    pub fn get_future(&self) -> &mut Pin<Box<dyn Future<Output = i32> + 'static>> {
        unsafe { self.fut.get().as_mut().unwrap() }
    }

    /// Whether the task has been inited
    #[inline]
    pub const fn is_init(&self) -> bool {
        self.is_init
    }

    /// Whether the task is the idle task
    #[inline]
    pub const fn is_idle(&self) -> bool {
        self.is_idle
    }

    /// Set the task waiting for reschedule
    #[inline]
    #[cfg(feature = "preempt")]
    pub fn set_preempt_pending(&self, pending: bool) {
        self.need_resched.store(pending, Ordering::Release)
    }

    /// Get whether the task is waiting for reschedule
    #[inline]
    #[cfg(feature = "preempt")]
    pub fn get_preempt_pending(&self) -> bool {
        self.need_resched.load(Ordering::Acquire)
    }

    /// Whether the task can be preempted
    #[inline]
    #[cfg(feature = "preempt")]
    pub fn can_preempt(&self) -> bool {
        self.preempt_disable_count.load(Ordering::Acquire) == 0
    }

    /// Disable the preemption
    #[inline]
    #[cfg(feature = "preempt")]
    pub fn disable_preempt(&self) {
        self.preempt_disable_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Enable the preemption by increasing the disable count
    ///
    /// Only when the count is zero, the task can be preempted
    #[inline]
    #[cfg(feature = "preempt")]
    pub fn enable_preempt(&self) {
        self.preempt_disable_count.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get the number of preempt disable counter
    #[inline]
    #[cfg(feature = "preempt")]
    pub fn preempt_num(&self) -> usize {
        self.preempt_disable_count.load(Ordering::Acquire)
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

    /// Reset the task time statistics
    pub fn reset_time_stat(&self, current_timestamp: usize) {
        let time = self.time.get();
        unsafe {
            (*time).reset(current_timestamp);
        }
    }

    /// Check whether the timer triggered
    ///
    /// If the timer has triggered, then reset it and return the signal number
    pub fn check_pending_signal(&self) -> Option<usize> {
        let time = self.time.get();
        unsafe { (*time).check_pending_timer_signal() }
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
