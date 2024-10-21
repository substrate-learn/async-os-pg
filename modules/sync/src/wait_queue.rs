use core::future::Future;
use core::task::{Context, Poll, Waker};
use spinlock::SpinNoIrq;
use crate::wait_list::WaitTaskList;
use crate::wait_list::WaitWakerNode;
use alloc::sync::Arc;

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

    /// 当前任务进入阻塞状态，将 cx 注册到等待队列中
    pub fn wait<'a>(&'a self) -> WaitFuture<'a> {
        WaitFuture { wq: self, flag: false }
    }

    /// 当前任务等待某个条件成功
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

    /// 当前任务等待，直到 deadline
    /// 参数使用 deadline，如果使用 Duration，则会导致每次进入这个函数都会重新计算 deadline
    /// 从而导致一直无法唤醒
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

    /// 当前任务等待条件满足或者到达deadline
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

pub struct WaitFuture<'a> {
    wq: &'a WaitQueue,
    flag: bool
}

impl<'a> Future for WaitFuture<'a> {
    type Output = ();

    fn poll(self: core::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker_node = Arc::new(WaitWakerNode::new(cx.waker().clone()));
        let Self { wq, flag } = self.get_mut();
        if !*flag {
            wq.queue.lock().prepare_to_wait(waker_node);
            Poll::Pending
        } else {
            wq.queue.lock().remove(&waker_node);
            Poll::Ready(())
        }
    }
}
