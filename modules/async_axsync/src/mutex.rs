//! A naïve sleeping mutex.

use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU64, Ordering};
use crate::WaitQueue;
use core::{future::Future, pin::Pin, task::{Context, Poll}};

extern "C" {
    fn current_task_id() -> u64;
}

/// A mutual exclusion primitive useful for protecting shared data, similar to
/// [`std::sync::Mutex`](https://doc.rust-lang.org/std/sync/struct.Mutex.html).
///
/// When the mutex is locked, the current task will block and be put into the
/// wait queue. When the mutex is unlocked, all tasks waiting on the queue
/// will be woken up.
pub struct Mutex<T: ?Sized> {
    wq: WaitQueue,
    owner_id: AtomicU64,
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
    data: *mut T,
}

unsafe impl<'a, T: ?Sized + 'a> Send for MutexGuardFuture<'a, T> {}
unsafe impl<'a, T: ?Sized + 'a> Sync for MutexGuardFuture<'a, T> {}

impl<'a, T: ?Sized + 'a> MutexGuardFuture<'a, T> {
    pub fn new(
        lock: &'a Mutex<T>,
        data: *mut T,
    ) -> Self {
        Self { lock, data }
    }
}

impl<'a, T: ?Sized + 'a> Future for MutexGuardFuture<'a, T> {
    type Output = MutexGuard<'a, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { lock, data } = self.get_mut();
        let current_id = unsafe { current_task_id() };
        match lock.owner_id.compare_exchange_weak(
            0,
            current_id,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => Poll::Ready(MutexGuard {
                lock: *lock,
                data: *data,
            }),
            Err(owner_id) => {
                assert_ne!(
                    owner_id, current_id,
                    "Thread({}) tried to acquire mutex it already owns.",
                    current_id,
                );
                lock.wq.wait_until(cx, || !lock.is_locked()).map(|_| {
                    MutexGuard {
                        lock: *lock,
                        data: *data,
                    }
                })
            }
        }
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
            owner_id: AtomicU64::new(0),
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
        self.owner_id.load(Ordering::Relaxed) != 0
    }

    /// Locks the [`Mutex`] and returns a guard that permits access to the inner data.
    ///
    /// The returned value may be dereferenced for data access
    /// and the lock will be dropped when the guard falls out of scope.
    pub fn lock(&self) -> MutexGuardFuture<T> {
        MutexGuardFuture::new(self, self.data.get())
    }

    /// Try to lock this [`Mutex`], returning a lock guard if successful.
    #[inline(always)]
    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        let current_id = unsafe { current_task_id() };
        // The reason for using a strong compare_exchange is explained here:
        // https://github.com/Amanieu/parking_lot/pull/207#issuecomment-575869107
        if self
            .owner_id
            .compare_exchange(0, current_id, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(MutexGuard {
                lock: self,
                data: unsafe { &mut *self.data.get() },
            })
        } else {
            None
        }
    }

    /// Force unlock the [`Mutex`].
    ///
    /// # Safety
    ///
    /// This is *extremely* unsafe if the lock is not held by the current
    /// thread. However, this can be useful in some instances for exposing
    /// the lock to FFI that doesn’t know how to deal with RAII.
    pub unsafe fn force_unlock(&self) {
        let owner_id = self.owner_id.swap(0, Ordering::Release);
        let current_id = current_task_id();
        
        assert_eq!(
            owner_id, current_id,
            "Thread({}) tried to release mutex it doesn't own",
            current_id,
        );
        // wake up one waiting thread.
        self.wq.notify_one();
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

impl<T: ?Sized + fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => write!(f, "Mutex {{ data: ")
                .and_then(|()| (*guard).fmt(f))
                .and_then(|()| write!(f, "}}")),
            None => write!(f, "Mutex {{ <locked> }}"),
        }
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
        unsafe { self.lock.force_unlock() }
    }
}
