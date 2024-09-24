# async-os

这个仓库借鉴了 [Arceos](https://github.com/arceos-org/arceos) 和 [Starry](https://github.com/Starry-OS/Starry) 的实现，尽可能的利用这两个仓库中已有的 crate 实现。在此基础上来使用协程来构建异步内核和用户态程序。

- crates 目录下的模块是未经过修改，直接使用的模块。
    - arch_boot
- modules 目录下的模块则是经过修改用于构建异步内核的实现