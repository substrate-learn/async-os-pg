# async_io

这个模块提供了关于 IO 的异步接口（Read、BufRead、Seek、Write）定义以及部分默认实现，以及异步迭代器（Stream/AsyncIterator）接口的定义以及实现。目前的实现大部分参考自 [async-std](https://github.com/async-rs/async-std/tree/main)、[future-rs](https://github.com/rust-lang/futures-rs) ，但目标是在内核中使用异步 IO 的特性。

## 异步 IO 接口

### Read trait

- read
- read_vectored
- read_exact
- read_to_end
- read_to_string

`Read trait` 基于 `AsyncRead trait`，若需要为一个新的对象实现 `Read trait`，则只需要为这个对象实现 `AsyncRead trait` 即可使用上述接口。

### BufRead

- read_line
- read_until

基于 `AsyncBufRead trait`

### Seek

- seek

基于 `AsyncSeek trait`

### Write

- write
- write_vectored
- flush
- write_all
- write_fmt

基于 `AsyncWrite trait`

## 异步迭代器

### Stream

这个实现直接复制了 [async-std](https://github.com/async-rs/async-std/tree/main) 中的实现，没有使用 `core::async_iter::AsyncIterator`，提供了异步迭代器
