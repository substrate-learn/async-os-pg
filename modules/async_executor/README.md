# async_executor

这个模块提供了异步运行时的 Executor，每个进程存在自己的 Executor，并且存在各自的与 Executor 相关的 Waker
内核与用户态的 Executor 存在不同的唤醒方法，这个仓库用于定义通用的唤醒方法，提供在内核与用户态程序的复用

基于该 Executor 可以构建嵌套的运行时，进程所属的 Executor 的 run 函数作为一个任务，来表示进入到用户态空间运行，这个任务由内核的 Executor 进行管理。

