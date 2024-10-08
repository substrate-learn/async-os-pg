# async_vfs

这个模块定义了 vfs 的接口。其中 AsyncVfsNodeOps 定义了与 VfsNode 相关的接口，AsyncVfsOps 定义了与 Vfs 相关的接口。这两个接口依赖于 VfsNodeOps、VfsOps 这两个基本的 trait。

采取这种实现是因为如果直接在 trait 的定义中使用 async 关键字来定义接口，在部分接口中可能返回的结果是 `Arc<dyn VfsNodeRef>`，编译器会提示出错。参考下列示例

```rust
type TestRef = Arc<dyn Test>;
trait Test {
    async fn test() -> Option<TestRef>;
}
```

编译器将报错：the trait `Test` cannot be made into an object。

并且参考 async-std 中关于 IO 的异步接口定义以及协程的实现，采取了这种方式，在为新的对象实现 `AsyncVfsNodeOps trait` 时，直接实现 `VfsNodeOps trait` 即可，从而可以使用异步的特性。

