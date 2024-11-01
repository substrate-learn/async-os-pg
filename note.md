# async-os学习研究笔记

## 问题汇总

> 问题：无法运行`make A=apps/helloworld ARCH=x86_64 run`

只能在riscv64架构上运行，请使用`ARCH=riscv64`

> 问题：尝试运行，然后报错：
>
> ```
> rust-objcopy --binary-architecture=riscv64 apps/helloworld/helloworld_riscv64-qemu-virt.elf --strip-all -O binary apps/helloworld/helloworld_riscv64-qemu-virt.bin
> make: rust-objcopy: No such file or directory
> make: *** [scripts/make/build.mk:46: apps/helloworld/helloworld_riscv64-qemu-virt.bin] Error 127
> ```

先`cargo install cargo-binutils`，再`rustup component add llvm-tools-preview`。如果出现其他报错，按照rCore_dev_guide处理。

> 问题：找不到块设备。

先运行`./build_img.sh -a riscv64`，再在`Makefile`中设置`BLK ?= y`，最后检查`DISK_IMG`的默认值是否为`disk.img`。或者直接在`make`指令中指定`BLK=y`也成。

> 问题：默认在Unikernel模式下，不知如何进入宏内核模式？

就此请教赵前辈，得到的信息以及验证情况如下：

- [x] 目前，只有`apps/user_boot`这个App能进入用户态。进入后，将通过文件系统读取用户程序的`ELF`文件，然后加载运行。
- [x] 运行上述App的`make`指令为`make A=apps/user_boot ARCH=riscv64 LOG=off SMP=1 FEATURES=sched_fifo,img  BLK=y run`。
- [ ] 想要令某个app使用用户态，则必须在其依赖项中加入`features = [ "monolithic" ]`
- [ ] 若要自行编写用户态App，需要遵循以下步骤：
  - [x] 参考rCore的用户态程序编写用户态代码，然后编译获得ELF文件。此时，工具链用 `riscv...unknown...elf` 的，并且可用Linux下的标准调用语法（个人理解：借助`libc` crate，将我们的操作翻译为系统调用）
  - [x] 将编译获得的ELF文件放在 `testcases/riscv64_linux_musl` 目录下
  - [x] 重新编译一次 `disk.img`

按ZFL前辈的说法，rCore那里编写的用户态程序是可以直接在async-os的用户态中运行的。于是，我们采取这样的做法：

1. 在rCore的 `user/` 目录下运行 `make build` 后，在 `user/target/riscv64gc-unknown-none-elf/release` 中找到了编译成ELF文件的 `hello_world` 。将其拷贝到 `testcases/riscv64_linux_musl` 目录下后，重新编译 `disk.img` （通过运行“找不到块设备”中的命令），执行上述 `make` 指令，没变化欸？？？
2. 观察 `apps/user_boot/src/main.rs` 的内容，发现它将测例的名字 `hello` 硬编码到代码中了，于是改名成 `hello_world` 再次编译，发现还是没变化欸？？？
3. 将日志等级开到 `LOG=info` 再次编译，发现报告了错误：
   ```no_run
   panicked at /home/endericedragon/repos/async-os/modules/trampoline/src/task_api.rs:139:21:
   Unhandled trap Exception(LoadPageFault) @ 0x100a8:
   TrapFrame { ... }
   ```
   似乎是在用户态出现了未能处理的异常。

排查工作至此卡住，询问ZFL前辈后得知两个内核向用户态传输参数的方式不同。只需将 `modules/taskctx/src/arch/riscv/mod.rs` 中的以下代码注释掉即可：

```rust
impl TrapFrame {
    /// 用于创建用户态任务的初始化
    pub fn init_user_context(app_entry: usize, user_sp: usize) -> Self {
        // -- snip --
        unsafe {
            // a0为参数个数
            // a1存储的是用户栈底，即argv
            trap_frame.regs.a0 = *(user_sp as *const usize);
            trap_frame.regs.a1 = *(user_sp as *const usize).add(1);
        }
        // -- snip --
    }
}
```

虽然问题暂时解决了，但是深层次的问题没有解决：

1. ZFL前辈是如何定位到这个问题的？
2. 有办法让rCore的用户程序编译完可以直接被async-os加载运行吗？

要解决第一个问题，赵前辈提出了一系列技术方法：

- 调试：GDB
- 反汇编：rust-objdump
- 模拟器日志：qemu log，搜到了一篇 [QEMU虚拟机日志调试](https://www.baeldung.com/linux/qemu-vm-logging-debugging) 博文，在async-os中可以通过 `QEMU_LOG=y` 启用
- 跟踪系统调用：strace

### 尝试自主定位问题

首先尝试采用阅读QEMU日志的方法进行故障排查。指定 `QEMU_LOG=y` 后再运行一次 `user_boot` ，然后查看 `qemu.log` 文件。由于我们从报错信息得知错误类型为 `LoadPageFault`，且出现错误的代码位于 `0x100a8`，因此可以搜索 `0x100a8` 相关的日志信息。

经过搜索，找到相关信息如下：

```
----------------
IN:
Priv: 0; Virt: 0
...
0x00000000000100a8:  00154503          lbu             a0,1(a0)  # <- Here
0x00000000000100ac:  0605              addi            a2,a2,1
0x00000000000100ae:  f97d              bnez            a0,-10          # 0x100a4

riscv_cpu_tlb_fill ad 6f775f6f6c6c6568 rw 0 mmu_idx 0
riscv_cpu_tlb_fill address=6f775f6f6c6c6568 ret 1 physical 0000000000000000 prot 0
riscv_cpu_do_interrupt: hart:0, async:0, cause:000000000000000d, epc:0x00000000000100a8, tval:0x6f775f6f6c6c6568, desc=load_page_fault
----------------
```

可以看到，`0x100a8` 处是一个 `LBU` 指令，其功能解释为，从内存地址 `a0 + 1` 中读取一个字节，然后放到 `a0` 寄存器中去。显然，`a0` 指向的内存地址有问题。

Fitten Code提示说，此时让反汇编介入可获得更多信息，于是尝试对 ELF 文件进行反汇编：

```sh
rust-objdump --disassemble-all testcases/riscv64_linux_musl/hello_world > disassemble.txt
```

然后再搜索 `100a8` ，获得的结果和QEMU LOG中的类似，一条LBU指令。

那么，我们可以还原出错前一小段时间内CPU里发生的事情：

```asm
1009e: 52 95        	add	a0, a0, s4  # 给a0加上s4的值
100a0: 0c 61        	ld	a1, 0x0(a0) # 将内存地址a0中的值放到a1中，注意到此时读内存是正常的
100a2: 7d 56        	li	a2, -0x1    # 给a2赋值为-1
100a4: 33 85 c5 00  	add	a0, a1, a2  # 给a0加上a1和a2的和
# 综合上述过程，其实相当于：a0 = *(a0 + 0) - 1
100a8: 03 45 15 00  	lbu	a0, 0x1(a0) # 炸了！
```

我们已经搞明白了出问题的指令是什么，但这个指令是谁发出的？在源代码中又体现在哪里呢？我们需要借助GDB一探究竟。

### GDB调试

书接上回，我们利用GDB尝试进行调试（找了篇[教程](https://www.cnblogs.com/lvdongjie/p/8994092.html)熟悉了一下常用命令）。调试需要二进制文件中具有调试信息，我们做如下更改：
- `scripts/make/build.mk` 中的 `_cargo_build` 命令，将 `--strip-all` 删除
- 调试时，使用如下指令： `make A=apps/user_boot ARCH=riscv64 MODE=debug LOG=info SMP=1 FEATURES=sched_fifo,img BLK=y mydebug`

其中的 `mydebug` 命令如下定义：

```makefile
mydebug: build
	$(call run_qemu_debug) &
	sleep 1
	$(GDB) $(OUT_ELF) \
	  -ex 'target remote localhost:1234'
```

## 代码阅读

### 异步的Mutex实现探究

既然目前的问题聚焦于`Mutex`之上，那我们不如就抓着`Mutex`这一条线，采取纵向阅读的方法，探究这个异步的`Mutex`到底是如何实现的呗？

我们知道这个`Mutex`来自`modules/sync/src/mutex.rs`，我们的阅读就从这里开始吧。

## 增添功能

### 新建模块async_std::collections

注意到当前操作系统没有提供容器数据结构，仅有`alloc::vec::Vec`，因此有必要增添`async_std::collections`来提供常用的容器数据结构。

本次功能增添一共新增两个容器：`HashMap`和`BinaryHeap`。前者直接通过引入`hashbrown`库实现，后者

### 尝试移植futures-bounded#0.2.3

这个库在rust-libp2p的依赖图拓扑排序中排在非常靠前的位置，因此先从它开始移植。
