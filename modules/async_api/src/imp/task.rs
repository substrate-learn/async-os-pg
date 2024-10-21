
pub async fn ax_sleep_until(deadline: crate::time::AxTimeValue) {
    trampoline::sleep_until(deadline).await;
}

pub async fn ax_yield_now() {
    trampoline::yield_now().await;
}

cfg_task! {
    use core::time::Duration;
    use core::{future::Future, pin::Pin, task::{Context, Poll}};
    use core::ops::Deref;

    /// A handle to a task.
    pub struct AxTaskHandle {
        inner: trampoline::TaskRef,
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
    pub struct AxWaitQueueHandle(sync::WaitQueue);

    impl AxWaitQueueHandle {
        /// Creates a new empty wait queue.
        pub const fn new() -> Self {
            Self(sync::WaitQueue::new())
        }
    }

    impl Deref for AxWaitQueueHandle {
        type Target = sync::WaitQueue;
        fn deref(&self) -> &Self::Target { 
            &self.0
        }
    }

    pub fn ax_current_task_id() -> u64 {
        trampoline::current_task().id().as_u64()
    }

    pub fn ax_spawn<F>(f: F, name: alloc::string::String) -> AxTaskHandle
    where
        F: Future<Output = i32> + 'static,
    {
        let inner = trampoline::spawn_raw(move || f, name);
        AxTaskHandle {
            id: inner.id().as_u64(),
            inner,
        }
    }

    pub fn ax_set_current_priority(prio: isize) -> crate::AxResult {
        if trampoline::set_priority(prio) {
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

    pub fn ax_wait_for_exit(task: AxTaskHandle) -> trampoline::JoinFuture {
        // task.inner.join()
        // axtask::join(&task.inner).await
        trampoline::join(&task.inner)
    }
    
    pub fn ax_wait_queue_wait(
        wq: &AxWaitQueueHandle,
        cx: &mut Context<'_>, 
        until_condition: impl Fn() -> bool,
        timeout: Option<Duration>,
    ) -> Poll<bool> {
        #[cfg(feature = "irq")]
        if let Some(dur) = timeout {
            let deadline = axhal::time::current_time() + dur;
            return wq.0.wait_timeout_until(cx, deadline, until_condition);
        }
        if timeout.is_some() {
            axlog::warn!("ax_wait_queue_wait: the `timeout` argument is ignored without the `irq` feature");
        }
        wq.0.wait_until(cx, until_condition).map(|_| false)
    }

}
