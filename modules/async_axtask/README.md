# axtask

## 设计

在原本的 axtask 基础上，直接增加与地址空间相关的功能。Executor 中增加地址空间（Process/kernel）的功能。

异步 IPC、异步系统调用转化为 Executor 之间的通信。

## task

在 taskctx 中提供的 TaskInner 的基础上，增加了 preempt_ctx 字段，用于记录在内核发生时钟中断进行抢占时，保存的上下文。



