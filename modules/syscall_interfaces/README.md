# syscall_interfaces

针对 async 和非 async 环境下，使用 async 和非 async 实现的系统调用接口的若干思考，这里将以 read 系统调用为例。

```rust
pub fn read(fd: usize, base: *mut u8, len: usize) -> isize {
    0
}
```

其中同步的接口如下所示：

```rust
/// 这个是同步系统调用的接口
pub fn sys_read(fd: usize, base: *mut u8, len: usize) -> isize {
    /// 执行系统调用进入内核
    0
}
```

异步接口如下所示：

```rust
/// 这个是异步系统调用的接口
pub fn sys_read_(fd: usize, base: *mut u8, len: usize) -> ReadFuture {
    /// 仅仅返回 Future 对象
    ReadFuture { fd, base, len }
}

use core::{future::Future, pin::Pin, task::{Context, Poll}};

pub struct ReadFuture {
    fd: usize,
    base: *mut u8,
    len: usize,
}

impl Future for ReadFuture {
    type Output = isize;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(0)
    }
}
```

以上两种形式，保证了系统调用的参数一致，接口的返回值类型不同，但 `ReadFuture` 通过 await 关键字执行后的返回值则与原来的相同。

## async 环境
若上层的函数为 async 关键字创建的函数，则两种接口都是适用的，例如：

```rust
pub async fn read(fd: usize, base: *mut u8, len: usize) -> isize {
    // 非 async 系统调用
    sys_read(fd, base, len)
    // async 系统调用
    sys_read_(fd, base, len).await
}
```

## 非 async 环境

考虑到与已有的程序的兼容性，上层的函数不是由 async 关键字创建的，在这种环境下，不能直接通过 await 使用 async 系统调用，例如：

```rust
pub fn read(fd: usize, base: *mut u8, len: usize) -> isize {
    // 非 async 系统调用
    sys_read(fd, base, len)
    // 这里不能直接使用
    // sys_read_(fd, base, len).await
    // 只能通过原始的 core::future::Future trait 定义的 poll 函数执行这个 ReadFuture
    // let res = Pin::new(&mut sys_read_(fd, base, len)).poll(cx);
    // match res {
    //     Poll::Ready(res) => return res,
    //     Poll::Pending => {
    //         // 让出 CPU，执行其他的任务
    //     },
    // }
}
```

使用 `Future trait` 提供的 poll 接口执行 `ReadFuture` 需要在这个函数中嵌入一个协程运行时，使得在系统调用无法马上获取结果时，让其他的协程能够继续执行。
但这里不能仅仅通过一个 loop 循环，当返回 Pending 时，取出其他协程去执行，因为之前的执行流还在函数栈上，栈没有被清空，若循环出现这种调用，则会导致栈溢出。

### 面临的问题

1. 将栈上的函数调用关系清空
2. 保证下一次从让出点恢复执行

仅仅将原来的同步函数使用 async 关键字包装起来可以实现函数调用关系清空，但是不保证下一次会从让出点执行。用户态的有栈协程可以保证下一次从让出点执行，但不能栈复用。完美的解决方案是需要对编译器进行修改，实现系统调用过滤。

## 结论

目前仅考虑在 async 环境下使用同步和异步的接口，并进行对比
