use core::task::{Context, Poll, Waker};
use spinlock::SpinNoIrq;
use crate::wait_list::WaitTaskList;
use crate::wait_list::WaitWakerNode;
use alloc::sync::Arc;

/// A queue to store sleeping tasks.
///
/// # Examples
///
/// ```
/// use axtask::WaitQueue;
/// use core::sync::atomic::{AtomicU32, Ordering};
///
/// static VALUE: AtomicU32 = AtomicU32::new(0);
/// static WQ: WaitQueue = WaitQueue::new();
///
/// axtask::init_scheduler();
/// // spawn a new task that updates `VALUE` and notifies the main task
/// axtask::spawn(|| {
///     assert_eq!(VALUE.load(Ordering::Relaxed), 0);
///     VALUE.fetch_add(1, Ordering::Relaxed);
///     WQ.notify_one(true); // wake up the main task
/// });
///
/// WQ.wait(); // block until `notify()` is called
/// assert_eq!(VALUE.load(Ordering::Relaxed), 1);
/// ```
///

#[macro_export]
macro_rules! declare_wait {
    ($name: ident) => {
        let $name = Arc::new(WaitWakerNode::new($crate::waker::waker_from_task(
            $crate::current().as_task_ref().clone()
        )));
    };
}

pub struct WaitQueue {
    // Support queue lock by external caller,use SpinNoIrq
    // Arceos SpinNoirq current implementation implies irq_save,
    // so it can be nested
    // use linked list has good performance
    queue: SpinNoIrq<WaitTaskList>,
}

impl WaitQueue {
    /// Creates an empty wait queue.
    pub const fn new() -> Self {
        Self {
            queue: SpinNoIrq::new(WaitTaskList::new()),
        }
    }

    pub fn wait(&self, cx: &mut Context<'_>, flag: bool) -> Poll<()> {
        let waker_node = Arc::new(WaitWakerNode::new(cx.waker().clone()));
        if !flag {
            self.queue.lock().prepare_to_wait(waker_node);
            Poll::Pending
        } else {
            self.queue.lock().remove(&waker_node);
            Poll::Ready(())
        }
    }

    pub fn wait_until(
        &self, 
        cx: &mut Context<'_>, 
        condition: impl Fn() -> bool
    ) -> Poll<()> {
        let waker_node = Arc::new(WaitWakerNode::new(cx.waker().clone()));
        if condition() {
            self.queue.lock().remove(&waker_node);
            Poll::Ready(())
        } else {
            self.queue.lock().prepare_to_wait(waker_node);
            Poll::Pending
        }
    }

    /// If the arg is duration, the deadline must be stored in somewhere. 
    /// Otherwise, the deadline will changed with the current_time.
    /// So the arg is the deadline.
    #[cfg(feature = "irq")]
    pub fn wait_timeout(
        &self, 
        cx: &mut Context<'_>, 
        deadline: axhal::time::TimeValue,
        flag: bool
    ) -> Poll<bool> {
        let waker_node = Arc::new(WaitWakerNode::new(cx.waker().clone()));
        if !flag {
            self.queue.lock().prepare_to_wait(waker_node);
            crate::timers::set_alarm_wakeup(deadline, cx.waker().clone());
            Poll::Pending
        } else {
            crate::timers::cancel_alarm(cx.waker());
            self.queue.lock().remove(&waker_node);
            Poll::Ready(axhal::time::current_time() >= deadline)
        }
    }

    #[cfg(feature = "irq")]
    pub fn wait_timeout_until(
        &self, 
        cx: &mut Context<'_>, 
        deadline: axhal::time::TimeValue,
        condition: impl Fn() -> bool
    ) -> Poll<bool>{
        let waker_node = Arc::new(WaitWakerNode::new(cx.waker().clone()));
        let current_time = axhal::time::current_time();
        if condition() {
            Poll::Ready(current_time >= deadline)
        } else {
            if current_time >= deadline {
                crate::timers::cancel_alarm(cx.waker());
                self.queue.lock().remove(&waker_node);
                Poll::Ready(true)
            } else {
                self.queue.lock().prepare_to_wait(waker_node);
                crate::timers::set_alarm_wakeup(deadline, cx.waker().clone());
                Poll::Pending
            }
        }
    }

    /// Wake up the given task in the wait queue.
    pub fn notify_task(&self, waker: &Waker) -> bool {
        self.queue.lock().notify_task(waker)
    }

    /// Wakes up one task in the wait queue, usually the first one.
    pub fn notify_one(&self) -> bool {
        self.queue.lock().notify_one()
    }

    /// Wakes all tasks in the wait queue.
    pub fn notify_all(&self) {
        self.queue.lock().notify_all()
    }
}
