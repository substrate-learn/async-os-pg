use core::{future::poll_fn, task::Poll};

pub use executor::*;
use taskctx::TrapStatus;
use riscv::register::scause::{Trap, Exception};
#[cfg(feature = "preempt")]
use crate::{trampoline, TrapFrame};

use crate::KERNEL_EXECUTOR;
pub fn turn_to_kernel_executor() {
    CurrentExecutor::clean_current();
    unsafe { CurrentExecutor::init_current(KERNEL_EXECUTOR.clone()) };
}

#[cfg(feature = "preempt")]
/// Checks if the current task should be preempted.
/// This api called after handle irq,it may be on a
/// disable_preempt ctx
pub fn current_check_preempt_pending(tf: &TrapFrame) {
    if let Some(curr) = current_task_may_uninit() {
        // if task is already exited or blocking,
        // no need preempt, they are rescheduling
        if curr.get_preempt_pending() && curr.can_preempt() && !curr.is_exited() && !curr.is_blocking()
        {
            debug!(
                "current {} is to be preempted , allow {}",
                curr.id_name(),
                curr.can_preempt()
            );
            preempt(curr.as_task_ref(), tf)
        }
    }    
}

#[cfg(feature = "preempt")]
/// Checks if the current task should be preempted.
/// This api called after handle irq,it may be on a
/// disable_preempt ctx
pub async fn current_check_user_preempt_pending(_tf: &TrapFrame) {
    if let Some(curr) = current_task_may_uninit() {
        // if task is already exited or blocking,
        // no need preempt, they are rescheduling
        if curr.get_preempt_pending() && curr.can_preempt() && !curr.is_exited() && !curr.is_blocking()
        {
            warn!(
                "current {} is to be preempted , allow {}",
                curr.id_name(),
                curr.can_preempt()
            );
            taskctx::CurrentTask::clean_current_without_drop();
            yield_now().await;
        }
    }    
}

#[cfg(feature = "preempt")]
pub fn preempt(task: &TaskRef, tf: &TrapFrame) {
    task.set_preempt_pending(false);
    task.set_preempt_ctx(tf);
    let new_kstack_top = taskctx::current_stack_top();
    taskctx::CurrentTask::clean_current_without_drop();
    let waker = taskctx::waker_from_task(task);
    waker.wake();
    unsafe {
        core::arch::asm!(
            "li a1, 0",
            "li a2, 0",
            "mv sp, {new_kstack_top}",
            "j  {trampoline}",
            new_kstack_top = in(reg) new_kstack_top,
            trampoline = sym trampoline,
        )
    }
}

#[cfg(feature = "preempt")]
pub fn restore_from_preempt_ctx(task: &TaskRef) {
    let mut preempt_ctx_lock = task.preempt_ctx_lock();
    if let Some(preempt_ctx) = preempt_ctx_lock.take() {
        // debug!("restore from preempt");
        let taskctx::PreemptCtx { kstack, trap_frame } = preempt_ctx;
        taskctx::put_prev_stack(kstack);
        drop(preempt_ctx_lock);
        unsafe { 
            (*trap_frame).preempt_return()
        };
    }
}

pub async fn wait(task: &TaskRef) -> Option<i32> {
    poll_fn(|cx| {
        if task.is_exited() {
            Poll::Ready(Some(task.get_exit_code()))
        } else {
            task.join(cx.waker().clone());
            Poll::Pending
        }
    }).await 
}

pub async fn user_task_top() -> i32 {
    loop {
        let curr = current_task();
        let mut tf = curr.utrap_frame().unwrap();
        if tf.trap_status == TrapStatus::Blocked {
            log::error!("handle user trap");
            let trap = tf.get_scause_type();
            match trap {
                Trap::Interrupt(_interrupt) => {
                    warn!("user task interrupt here");
                    crate::handle_user_irq(tf.get_scause_code(), &mut tf).await;
                    warn!("user task interrupt done");
                },
                Trap::Exception(Exception::UserEnvCall) => {
                    warn!("user ecall");
                    async_axhal::arch::enable_irqs();
                    tf.sepc += 4;
                    let result = syscall::trap::handle_syscall(
                        tf.regs.a7,
                        [
                            tf.regs.a0, tf.regs.a1, tf.regs.a2, tf.regs.a3, tf.regs.a4, tf.regs.a5,
                        ],
                    ).await;
                    // 单独处理 exit 系统调用
                    if tf.regs.a7 == syscall::TaskSyscallId::EXIT as usize {
                        return tf.regs.a0 as i32;
                    }
                    if -result == syscall::SyscallError::ERESTART as isize {
                        // Restart the syscall
                        tf.rewind_pc();
                    } else {
                        tf.regs.a0 = result as usize;
                    }
                    async_axhal::arch::disable_irqs();
                    warn!("user ecall end");
                }
                Trap::Exception(_exception) => {
                    // handle exception
                    panic!(
                        "Unhandled trap {:?} @ {:#x}:\n{:#x?}",
                        tf.get_scause_type(),
                        tf.sepc,
                        tf
                    );
                }
            }
            tf.trap_status = TrapStatus::Done;
        } 
        poll_fn(|_cx| {
            if tf.trap_status == TrapStatus::Done {
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        }).await
    }
}

