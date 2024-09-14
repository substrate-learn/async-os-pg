use core::{task::{Poll, Waker, Context}, future::Future, pin::Pin};
use spinlock::SpinNoIrq;
use crate::{wait_list::WaitTaskList, SleepFuture};
use crate::wait_list::WaitWakerNode;
use alloc::{sync::Arc, boxed::Box};
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

pub struct WaitFuture<'a> {
    wait_queue: &'a WaitQueue,
    polled_once: bool,
}

impl<'a> WaitFuture<'a> {
    pub fn new(wait_queue: &'a WaitQueue) -> Self {
        Self {
            wait_queue,
            polled_once: false,
        }
    }
}

impl<'a> Future for WaitFuture<'a> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker_node = Arc::new(WaitWakerNode::new(cx.waker().clone()));
        let Self { wait_queue, polled_once} = self.get_mut();
        if !*polled_once {
            *polled_once = true;
            wait_queue.queue.lock().prepare_to_wait(waker_node);
            Poll::Pending
        } else {
            wait_queue.queue.lock().remove(&waker_node);
            Poll::Ready(())
        }
    }
}

pub struct WaitUntilFuture<'a> {
    wait_queue: &'a WaitQueue,
    condition: *mut dyn FnOnce() -> bool,
}

impl<'a> WaitUntilFuture<'a> {
    pub fn new<F: FnOnce() -> bool>(wait_queue: &'a WaitQueue, condition: F) -> Self {
        Self {
            wait_queue,
            condition: Box::into_raw(Box::new(condition)),
        }
    }
}

impl<'a> Future for WaitUntilFuture<'a> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker_node = Arc::new(WaitWakerNode::new(cx.waker().clone()));
        let Self { wait_queue, condition} = self.get_mut();
        // let wait_queue = self.wait_queue;
        // let condition = Pin::new(Box::new(self.condition));
        if unsafe { Box::from_raw(*condition)() } {
            wait_queue.queue.lock().remove(&waker_node);
            Poll::Ready(())
        } else {
            wait_queue.queue.lock().prepare_to_wait(waker_node);
            Poll::Pending
        }
    }
}

pub struct WaitTimeoutFuture {
    wait_queue: Arc<WaitQueue>,
    sleep_future: SleepFuture,
}

impl WaitTimeoutFuture {
    pub fn new(wait_queue: Arc<WaitQueue>, dur: core::time::Duration) -> Self {
        Self {
            wait_queue,
            sleep_future: SleepFuture::new(axhal::time::current_time() + dur),
        }
    }
}

impl Future for WaitTimeoutFuture {
    type Output = bool;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker_node = Arc::new(WaitWakerNode::new(cx.waker().clone()));
        let Self { wait_queue, sleep_future } = self.get_mut();
        match Pin::new(sleep_future).as_mut().poll(cx) {
            Poll::Ready(timeout) => {
                wait_queue.queue.lock().remove(&waker_node);
                Poll::Ready(timeout)
            },
            Poll::Pending => {
                wait_queue.queue.lock().prepare_to_wait(waker_node);
                Poll::Pending
            },
        }
    }
}

pub struct WaitTimeoutUntilFuture {
    wait_queue: Arc<WaitQueue>,
    sleep_future: SleepFuture,
    deadline: core::time::Duration,
    condition: fn() -> bool,
}

impl WaitTimeoutUntilFuture {
    pub fn new(wait_queue: Arc<WaitQueue>, dur: core::time::Duration, condition: fn() -> bool) -> Self {
        let deadline = axhal::time::current_time() + dur;
        Self {
            wait_queue,
            sleep_future: SleepFuture::new(deadline),
            deadline,
            condition,
        }
    }
}

impl Future for WaitTimeoutUntilFuture {
    type Output = bool;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker_node = Arc::new(WaitWakerNode::new(cx.waker().clone()));
        let Self { 
            wait_queue, 
            sleep_future, 
            deadline,
            condition 
        } = self.get_mut();
        if !condition() {
            match Pin::new(sleep_future).as_mut().poll(cx) {
                Poll::Ready(timeout) => {
                    if timeout {
                        wait_queue.queue.lock().remove(&waker_node);
                        Poll::Ready(true)
                    } else {
                        wait_queue.queue.lock().prepare_to_wait(waker_node);
                        Poll::Pending
                    }
                },
                Poll::Pending => {
                    wait_queue.queue.lock().prepare_to_wait(waker_node);
                    Poll::Pending
                },
            }
        } else {
            wait_queue.queue.lock().remove(&waker_node);
            return Poll::Ready(axhal::time::current_time() >= *deadline);
        }
    }
}

pub struct WaitQueue {
    // Support queue lock by external caller,use SpinNoIrq
    // Arceos SpinNoirq current implementation implies irq_save,
    // so it can be nested
    // use linked list has good performance
    pub(crate) queue: SpinNoIrq<WaitTaskList>,
}

impl WaitQueue {
    /// Creates an empty wait queue.
    pub const fn new() -> Self {
        Self {
            queue: SpinNoIrq::new(WaitTaskList::new()),
        }
    }

    pub fn wait(&self) -> WaitFuture {
        WaitFuture::new(self)
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
