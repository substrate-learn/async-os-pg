use core::future::Future;

pub async fn ax_sleep_until(deadline: crate::time::AxTimeValue) {
    axtask::sleep_until(deadline).await;
}

pub async fn ax_yield_now() {
    axtask::yield_now().await;
}

cfg_task! {
    use core::time::Duration;

    /// A handle to a task.
    pub struct AxTaskHandle {
        inner: axtask::AxTaskRef,
        id: u64,
    }

    impl AxTaskHandle {
        /// Returns the task ID.
        pub fn id(&self) -> u64 {
            self.id
        }
    }

    /// A handle to a wait queue.
    ///
    /// A wait queue is used to store sleeping tasks waiting for a certain event
    /// to happen.
    pub struct AxWaitQueueHandle(axtask::WaitQueue);

    impl AxWaitQueueHandle {
        /// Creates a new empty wait queue.
        pub const fn new() -> Self {
            Self(axtask::WaitQueue::new())
        }
    }

    pub struct AxWaitExitFuture {
        inner: axtask::JoinFuture
    }

    impl AxWaitExitFuture {
        pub fn new(inner: axtask::JoinFuture) -> Self {
            Self { inner }
        }
    }

    pub struct AxWaitQueueFuture {
        #[cfg(feature = "irq")]
        inner: axtask::WaitTimeoutUntilFuture,
        #[cfg(not(feature = "irq"))]
        inner: axtask::WaitUntilFuture
    }

    impl AxWaitQueueFuture {
        #[cfg(feature = "irq")]
        pub fn new(inner: axtask::WaitTimeoutUntilFuture) -> Self {
            Self { inner }
        }

        #[cfg(not(feature = "irq"))]
        pub fn new(inner: axtask::WaitUntilFuture) -> Self {
            Self { inner }
        }
    }

    pub fn ax_current_task_id() -> u64 {
        axtask::current().id().as_u64()
    }

    pub fn ax_spawn<F>(f: F, name: alloc::string::String, stack_size: usize) -> AxTaskHandle
    where
        F: Future<Output = i32> + 'static + Send,
    {
        let inner = axtask::spawn_raw(move || f, name, stack_size);
        AxTaskHandle {
            id: inner.id().as_u64(),
            inner,
        }
    }

    pub fn ax_set_current_priority(prio: isize) -> crate::AxResult {
        if axtask::set_priority(prio) {
            Ok(())
        } else {
            axerrno::ax_err!(
                BadState,
                "ax_set_current_priority: failed to set task priority"
            )
        }
    }

    pub fn ax_wait_queue_wake(wq: &AxWaitQueueHandle, count: u32) {
        if count == u32::MAX {
            wq.0.notify_all();
        } else {
            for _ in 0..count {
                wq.0.notify_one();
            }
        }
    }

    pub fn ax_wait_for_exit(task: AxTaskHandle) -> AxWaitExitFuture {
        // task.inner.join()
        // axtask::join(&task.inner).await
        AxWaitExitFuture::new(axtask::join(&task.inner))
    }
    
    pub fn ax_wait_queue_wait(
        wq: &AxWaitQueueHandle,
        until_condition: impl Fn() -> bool,
        timeout: Option<Duration>,
    ) -> AxWaitQueueFuture {
        #[cfg(feature = "irq")]
        if let Some(dur) = timeout {
            // return wq.0.wait_timeout_until(dur, until_condition).await;
            let inner = axtask::WaitTimeoutUntilFuture::new(wq.0.clone(), dur, until_condition);
            return AxWaitQueueFuture { inner };
        }
    
        if timeout.is_some() {
            axlog::warn!("ax_wait_queue_wait: the `timeout` argument is ignored without the `irq` feature");
        }
        let inner = axtask::WaitUntilFuture::new(wq.0.clone(), until_condition);
        wq.0.wait_until(until_condition).await;
        false
    }

}
