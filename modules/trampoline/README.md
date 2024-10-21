# trampoline

需要进行执行流切换时的入口

执行流、任务切换的跳板页

## Trap

在发生 trap 时，只能使用 sscratch 寄存器，目前的做法是在 sscratch 寄存器中存放内核栈的栈顶，如果已经在内核中，则不会写这个寄存器。

按照目前的设计，sscratch 寄存器中需要记录 trap_frame 的指针，当发生中断时，上下文直接保存在指定的位置。

目前的问题，在初始化第一次产生时钟中断时，是没有设置好任务的，因此这时的上下文只能保存当前栈上，并进行时钟中断的处理

关于 TrapFrame 的定义：

```rust
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct TrapFrame {
    /// All general registers.
    pub regs: GeneralRegisters,
    /// Supervisor Exception Program Counter.
    pub sepc: usize,
    /// Supervisor Status Register.
    pub sstatus: usize,
    /// 浮点数寄存器
    pub fs: [usize; 2],
    /// Supervisor Cause Register
    pub scause: usize,
    /// Supervisor Trap Value
    pub stval: usize,
    /// 表示是否需要进行处理
    pub trap_status: TrapStatus,
    /// 备用的内核栈
    pub kernel_stack: usize,
}
```

这个内核栈怎么进行维护？这个备用的栈指针记录在 TrapFrame 中，而 sscratch 寄存器则记录 TrapFrame 的指针，当发生中断时（无论是内核还是用户态，都记录在这个位置），因此在运行前需要设置 sscratch 寄存器。当没有任务时，sscratch 寄存器为 0。

无论是什么任务，都将 gp、tp 寄存器保存在 TrapFrame 中，内核陷入到内核态，这种情况不需要切换 gp、tp，但从用户态陷入内核需要进行切换，这种情况如何进行区分呢？只有一个 sscratch 寄存器，怎么进行区分？将内核的 gp、tp 寄存器都保存在 TrapFrame 中，无论是何种陷入，都先将原来的 gp、tp 寄存器保存起来，再恢复内核的 gp、tp 寄存器。在中断处理过程中使用的内核栈如何获取，预先记录在 TrapFrame 中，但只有在运行之前才会记录

1. 无论是内核任务还是用户态任务，都需要将 sscratch 寄存器设置为 TrapFrame 的地址，在运行结束后将其清空
2. 



如果内核不手动对 sscratch 寄存器进行修改，在内核中是可以使用这个寄存器的。


## 用户态任务的组成、生命周期管理

用户态任务的组成结构：

1. 在用户态的执行流，从内核的视角看，即 trap_frame
2. 内核中的处理：
   1. 系统调用处理流程：参数为 trap_frame 中的 a0 ~ a7 寄存器
   2. 异常处理流程：参数为 scause、stval 寄存器中的信息
   3. 中断处理：参数为 scause 寄存器中的信息
   4. 信号处理

例如，在 [Phoenix](https://gitlab.eduxiji.net/educg-group-26010-2376550/T202418123993075-2940)（操作系统能力大赛一等奖） 的实现中，它将用户态任务的行为以如下方式实现：

```rust
pub async fn task_loop(task: Arc<Task>) {
    *task.waker() = Some(get_waker().await);
    loop {
        match task.state() {
            Terminated => break,
            Stopped => suspend_now().await,
            _ => {}
        }
        trap::user_trap::trap_return(&task);
        // task may be set to terminated by other task, e.g. execve will 
        // kill other tasks in the same thread group
        match task.state() {
            Terminated => break,
            Stopped => suspend_now().await,
            _ => {}
        }
        let intr = trap::user_trap::trap_handler(&task).await;
        match task.state() {
            Terminated => break,
            Stopped => suspend_now().await,
            _ => {}
        }
        do_signal(&task, intr).expect("do signal error");
    }
    task.do_exit();
}
```

它的内核态用户态切换是通过 trap_return 函数实现，TrapContext 是其中的关键数据，其结构如下：

```rust
pub struct TrapContext {
    // NOTE:  User to kernel should save these:
    /// General regs from x0 to x31.
    pub user_x: [usize; 32],
    /// CSR sstatus
    pub sstatus: Sstatus, // 32
    /// CSR sepc
    pub sepc: usize, // 33
    // NOTE: Kernel to user should save these:
    pub kernel_sp: usize, // 34
    pub kernel_ra: usize, // 35
    pub kernel_s: [usize; 12], // 36 - 47
    pub kernel_fp: usize, // 48
    pub kernel_tp: usize, // 49
    /// Float regs
    pub user_fx: UserFloatContext,
    /// used for the `SA_RESTART` flag of signal 
    pub last_a0: usize,
}
```

除了用户态执行流的整套寄存器上下文，还包括了内核中的执行流，这实际上就把内核的执行流转变成了线程，与 rCore 的做法没有什么区别。通过 loop 来实现循环，当 trap_return 时，会将内核执行流的上下文保存在 TrapContext 中的 kernel_sp、kernel_ra、kernel_s、kernel_fp、kernel_tp 等寄存器中，再从 user_x 中恢复用户态执行流上下文。当通过系统调用（中断、异常）进入内核态时，会将用户态执行流上下文保存在 user_x 中，从 kernel_sp、kernel_ra、kernel_s、kernel_fp、kernel_tp 中恢复内核的执行流，从而进入到下一阶段的 trap_handler 进行处理，当处理完成后，通过 loop 进入新一轮的循环。

我们的实现如下：

用户态任务对应的结构如下：

```rust
pub struct UserTask {
    trap_frame: Trapframe,
    scause: Scause,
    stval: Stval,
    need_resched: bool,
    exited: bool
}

impl UserTask {
    pub async fn run(&mut self) -> i32 {
        match self.scause {
            Trap::Exception(E::Breakpoint) => handle_breakpoint(&mut tf.sepc),
            Trap::Interrupt(_) => handle_irq(scause.bits(), from_user, tf),
            #[cfg(feature = "monolithic")]
            Trap::Exception(E::UserEnvCall) => {
                let (syscall_id, args) = self.trap_frame.get_syscall_arg();
                /// 返回 pending，则意味着系统调用的处理进入了阻塞状态，需要切换到其他任务
                /// 返回 Ready，则意味着系统调用处理完成，可以返回用户态执行。
                let res = syscall(syscall_id, args).await;
            }
            #[cfg(feature = "monolithic")]
            Trap::Exception(E::InstructionPageFault) => { }
            #[cfg(feature = "monolithic")]
            Trap::Exception(E::LoadPageFault) => { }
            #[cfg(feature = "monolithic")]
            Trap::Exception(E::StorePageFault) => { }

            _ => {
                panic!(
                    "Unhandled trap {:?} @ {:#x}:\n{:#x?}",
                    scause.cause(),
                    tf.sepc,
                    tf
                );
            }
        }
        /// 从 trap_frame 中恢复用户态的上下文
        if exited {
            self.trap_frame.exit_code
        } else if need_resched {
            // 返回 pending，意味着这个任务没有结束，需要让权
            poll_fn(|cx| Poll::Pending).await
        } else {
            /// trap_return 永远不会返回 Ready，用户任务的生命周期手动的拼接到一起，而不是通过保存内核态的上下文将其拼接起来
            /// 其他的在内核中使用协程的方式是通过 task_loop 以及保存内核态执行流的上下文的方式进行，
            /// 虽然他们可以做到内核中的任务复用同一个栈，但进出内核态需要保存的上下文数量是这种方式的两倍
            /// 
            /// 核心思路：内核中的执行流：例如 syscall、interrupt、exception 的处理都是以协程的方式进行，在处理的过程中已经把
            /// 上下文（指需要用到的变量，而不是寄存器）保存在堆上了，因此在执行 trap_return 时，栈上的那些函数调用关系以及局部变量
            /// 已经没有意义了，所以内核执行流的寄存器不需要保存
            /// 当用户任务通过 trap 进入到内核态时，这时不需要从栈上恢复上下文，而是直接拿到栈顶的指针，直接开始运行这个任务
            /// 因为任务内部的实现逻辑是先处理中断、异常、系统调用，正好构成一个循环
            trap_return(&self.trap_fame).await
        }
    }
}
```

run 函数中没有显式的使用 loop 来进行循环，但调整了 trap_return 的位置，放在末尾，当进入用户态时，此时 run 函数的函数调用栈中的内容已经没有意义。因此不需要保存内核执行流的寄存器上下文，只需要记录内核栈顶即可。当进入到 trap 的汇编函数时，会将用户态执行流的寄存器上下文保存在 trap_frame 中，并且将从之前记录的内核栈顶开始重新执行该 run 函数，从而实现循环。

用户态的任务对应了一个内核的 Trap 处理 Future，这个 Future 的生命周期对应着用户态任务的生命周期，当执行 exit 系统调用结束这个 Future 时，这意味着对应的用户态任务也结束。


代码中 CurrentTask、Waker、以及 抢占时，引用计数的关系应该如何处理，这实际上在写代码之前就应该考虑清楚，而不是在写了之后根据问题进行调试。



