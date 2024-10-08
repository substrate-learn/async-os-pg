# async_sync

这个仓库基于协程提供了 Mutex、WaitQueue、Timer 等实现。如果存在其他的任务正在占用锁，会主动让权，并且将 waker 注册到 WaitQueue 中。

其中，Mutex 的实现不再依赖任务 ID（这样需要依赖任务管理模块的 current 接口），在这里直接使用了 Waker 作为锁是否被任务占用的依据。
