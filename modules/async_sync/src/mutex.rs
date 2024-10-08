//! 只能在 async 函数以及自定义 Poll 函数中使用的 Mutex 实现。
//! 
//! 该 Mutex 以协程的方式实现，去掉了 force_lock 函数，
//! 因为协作式不存在强制的释放。
//! 
//! 去掉了 try_lock，因为 try_lock 本身也是一种协作的方式。
//! 当被锁上时，不等待，暂时去处理其他的事情，
//! 而这里的实现本身就是协作的方式，因此提供这个函数没有意义

use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::WaitQueue;
use core::{future::Future, pin::Pin, task::{Context, Poll}};

/// A mutual exclusion primitive useful for protecting shared data, similar to
/// [`std::sync::Mutex`](https://doc.rust-lang.org/std/sync/struct.Mutex.html).
///
/// When the mutex is locked, the current task will block and be put into the
/// wait queue. When the mutex is unlocked, all tasks waiting on the queue
/// will be woken up.
pub struct Mutex<T: ?Sized> {
    wq: WaitQueue,
    owner_task: AtomicUsize,
    data: UnsafeCell<T>,
}

/// A guard that provides mutable data access.
///
/// When the guard falls out of scope it will release the lock.
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a Mutex<T>,
    data: *mut T,
}

unsafe impl<'a, T: ?Sized + 'a> Send for MutexGuard<'a, T> {}

pub struct MutexGuardFuture<'a, T: ?Sized + 'a> {
    lock: &'a Mutex<T>,
}

unsafe impl<'a, T: ?Sized + 'a> Send for MutexGuardFuture<'a, T> {}
unsafe impl<'a, T: ?Sized + 'a> Sync for MutexGuardFuture<'a, T> {}

impl<'a, T: ?Sized + 'a> MutexGuardFuture<'a, T> {
    pub fn new(
        lock: &'a Mutex<T>,
    ) -> Self {
        Self { lock }
    }
}

impl<'a, T: ?Sized + 'a> Future for MutexGuardFuture<'a, T> {
    type Output = MutexGuard<'a, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { lock } = self.get_mut();
        lock.poll_lock(cx)
    }
}

// Same unsafe impls as `std::sync::Mutex`
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}

impl<T> Mutex<T> {
    /// Creates a new [`Mutex`] wrapping the supplied data.
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        Self {
            wq: WaitQueue::new(),
            owner_task: AtomicUsize::new(0),
            data: UnsafeCell::new(data),
        }
    }

    /// Consumes this [`Mutex`] and unwraps the underlying data.
    #[inline(always)]
    pub fn into_inner(self) -> T {
        // We know statically that there are no outstanding references to
        // `self` so there's no need to lock.
        let Mutex { data, .. } = self;
        data.into_inner()
    }
}

impl<T: ?Sized> Mutex<T> {
    /// Returns `true` if the lock is currently held.
    ///
    /// # Safety
    ///
    /// This function provides no synchronization guarantees and so its result should be considered 'out of date'
    /// the instant it is called. Do not use it for synchronization purposes. However, it may be useful as a heuristic.
    #[inline(always)]
    pub fn is_locked(&self) -> bool {
        self.owner_task.load(Ordering::Relaxed) != 0
    }

    /// Locks the [`Mutex`] and returns a guard that permits access to the inner data.
    ///
    /// The returned value may be dereferenced for data access
    /// and the lock will be dropped when the guard falls out of scope.
    pub fn lock(&self) -> MutexGuardFuture<T> {
        MutexGuardFuture::new(self)
    }

    /// 这个函数是底层的实现，用于在 Poll 函数中使用，而不是在 async 函数中使用
    /// 不仅仅是 MutexGuardFuture 中，在其他的使用到了 Mutex 的数据结构中，
    /// 在为它们实现 Future trait 或者自定义 Poll 函数时，需要使用这个接口
    pub fn poll_lock<'a>(&'a self, cx: &mut Context<'_>) -> Poll<MutexGuard<'a, T>> {
        let current_task = cx.waker().as_raw().data() as usize;
        match self.owner_task.compare_exchange_weak(
            0,
            current_task,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => Poll::Ready(MutexGuard {
                lock: self,
                data: self.data.get(),
            }),
            Err(owner_task) => {
                assert_ne!(
                    owner_task, current_task,
                    "Task({:#X}) tried to acquire mutex it already owns.",
                    owner_task,
                );
                // 当前线程让权，并将 cx 注册到等待队列上
                let a = self.wq.wait_until(cx, || !self.is_locked());
                // 进入这个分支一定是属于 Poll::Pending 的情况
                assert_eq!(&a, &Poll::Pending);
                Poll::Pending
            }
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the [`Mutex`] mutably, and a mutable reference is guaranteed to be exclusive in
    /// Rust, no actual locking needs to take place -- the mutable borrow statically guarantees no locks exist. As
    /// such, this is a 'zero-cost' operation.
    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        // We know statically that there are no other references to `self`, so
        // there's no need to lock the inner mutex.
        unsafe { &mut *self.data.get() }
    }
}

impl<T: ?Sized + Default> Default for Mutex<T> {
    #[inline(always)]
    fn default() -> Self {
        Self::new(Default::default())
    }
}

/// 这里的实现是 unsafe 的，不会获取锁，而是直接打印出数据
impl<T: ?Sized + fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Mutex {{ data: ")
            .and_then(|()| unsafe { self.data.get().as_ref().unwrap().fmt(f) })
            .and_then(|()| write!(f, "}}"))
    }
}

impl<'a, T: ?Sized> Deref for MutexGuard<'a, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        // We know statically that only we are referencing data
        unsafe { &*self.data }
    }
}

impl<'a, T: ?Sized> DerefMut for MutexGuard<'a, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        // We know statically that only we are referencing data
        unsafe { &mut *self.data }
    }
}

impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for MutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> Drop for MutexGuard<'a, T> {
    /// The dropping of the [`MutexGuard`] will release the lock it was created from.
    fn drop(&mut self) {
        self.lock.owner_task.swap(0, Ordering::Release);
        self.lock.wq.notify_one();
    }
}
