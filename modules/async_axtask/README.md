# axtask

## 设计

在原本的 axtask 基础上，直接增加与地址空间相关的功能。Executor 中增加地址空间（Process/kernel）的功能。

异步 IPC、异步系统调用转化为 Executor 之间的通信。

## task

在 taskctx 中提供的 TaskInner 的基础上，增加了 preempt_ctx 字段，用于记录在内核发生时钟中断进行抢占时，保存的上下文。

## 原则

使用 waker.wake 方法来替换原本的在调度代码中的放入就绪队列的操作，因此需要在调用之前完成所有对上下文的修改，这里的上下文不仅仅是寄存器上下文，还包括一些属性字段
