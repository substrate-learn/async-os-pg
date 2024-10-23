use core::sync::atomic::AtomicI32;

use alloc::{boxed::Box, format, string::String, sync::Arc, vec, vec::Vec};
use async_mem::MemorySet;
use axerrno::{AxError, AxResult};
use axsignal::signal_no::SignalNo;
use executor::{current_task, flags::CloneFlags, load_app, Executor, FdTable, SignalModule, Stderr, Stdin, Stdout, KERNEL_EXECUTOR_ID, PID2PC, TID2TASK};
use sync::Mutex;
use taskctx::{BaseScheduler, Task, TaskId, TaskInner, TaskRef, TrapFrame};
use async_axhal::{mem::VirtAddr, time::current_time_nanos};
use async_fs::api::OpenFlags;


extern "C" {
    fn start_signal_trampoline();
}

/// 根据给定参数创建一个新的 Executor
/// 在这期间如果，如果任务从一个核切换到另一个核就会导致地址空间不正确，产生内核页错误
pub async fn init_user(args: Vec<String>, envs: &Vec<String>) -> AxResult<TaskRef> {
    let mut path = args[0].clone();
    let mut memory_set = MemorySet::new_memory_set();
    {
        use async_axhal::mem::virt_to_phys;
        use async_axhal::paging::MappingFlags;
        // 生成信号跳板
        let signal_trampoline_vaddr: VirtAddr = (axconfig::SIGNAL_TRAMPOLINE).into();
        let signal_trampoline_paddr = virt_to_phys((start_signal_trampoline as usize).into());
        memory_set.map_page_without_alloc(
            signal_trampoline_vaddr,
            signal_trampoline_paddr,
            MappingFlags::READ
                | MappingFlags::EXECUTE
                | MappingFlags::USER
                | MappingFlags::WRITE,
        )?;
    }
    let page_table_token = memory_set.page_table_token();
    if page_table_token != 0 {
        unsafe {
            async_axhal::arch::write_page_table_root0(page_table_token.into());
            #[cfg(target_arch = "riscv64")]
            riscv::register::sstatus::set_sum();
        };
    }
    log::debug!("write page table done");
    let (entry, user_stack_bottom, heap_bottom) =
        if let Ok(ans) = load_app(path.clone(), args, envs, &mut memory_set).await {
            ans
        } else {
            error!("Failed to load app {}", path);
            return Err(AxError::NotFound);
        };
    let new_fd_table: FdTable = Arc::new(Mutex::new(vec![
        // 标准输入
        Some(Arc::new(Stdin {
            flags: Mutex::new(OpenFlags::empty()),
        })),
        // 标准输出
        Some(Arc::new(Stdout {
            flags: Mutex::new(OpenFlags::empty()),
        })),
        // 标准错误
        Some(Arc::new(Stderr {
            flags: Mutex::new(OpenFlags::empty()),
        })),
    ]));
    let new_executor = Arc::new(Executor::new(
        TaskId::new(),
        KERNEL_EXECUTOR_ID,
        Arc::new(Mutex::new(memory_set)),
        heap_bottom.as_usize() as u64,
        new_fd_table,
        Arc::new(Mutex::new(String::from("/").into())),
        Arc::new(AtomicI32::new(0o022)),
    ));
    if !path.starts_with('/') {
        //如果path不是绝对路径, 则加上当前工作目录
        let cwd = new_executor.get_cwd().await;
        assert!(cwd.ends_with('/'));
        path = format!("{}{}", cwd, path);
    }
    new_executor.set_file_path(path.clone()).await;
    let scheduler = new_executor.get_scheduler();
    let fut = Box::pin(crate::user_task_top());
    let pid = new_executor.pid().as_u64();
    let new_task = Arc::new(Task::new(
        TaskInner::new_user(
            path,
            pid,
            scheduler, 
            fut,
            TrapFrame::init_user_context(
                entry.into(), user_stack_bottom.into()
            )
        )
    ));

    // let new_task = spawn_raw(|| run_user_task(entry), path);
    // let new_task = new_task(Box::pin(UserTask::new(entry, user_stack_bottom)), path);
    // Executor::add_task(new_task.clone());
    new_executor.get_scheduler().lock().add_task(new_task.clone());
    TID2TASK
        .lock().await
        .insert(new_task.id().as_u64(), Arc::clone(&new_task));
    new_task.set_leader(true);

    new_executor
        .signal_modules
        .lock()
        .await
        .insert(new_task.id().as_u64(), SignalModule::init_signal(None));
    // new_process
    //     .robust_list
    //     .lock()
    //     .insert(new_task.id().as_u64(), FutexRobustList::default());
    PID2PC
        .lock().await
        .insert(new_executor.pid().as_u64(), Arc::clone(&new_executor));
    // // 将其作为内核进程的子进程
    // match PID2PC.lock().await.get(&KERNEL_PROCESS_ID) {
    //     Some(kernel_process) => {
    //         kernel_process.children.lock().await.push(new_process);
    //     }
    //     None => {
    //         return Err(Error::NotFound);
    //     }
    // }
    // spawn_raw(|| new_executor.run_self(), "executor".into());
    crate::spawn_raw(|| new_executor.run(), "executor".into());

    Ok(new_task)
}

/// 实现简易的clone系统调用
/// 返回值为新产生的任务的id
pub async fn clone_task(
    executor: &Arc<Executor>,
    flags: usize,
    stack: Option<usize>,
    ptid: usize,
    _tls: usize,
    ctid: usize,
    exit_signal: Option<SignalNo>,
) -> AxResult<u64> {
    let clone_flags = CloneFlags::from_bits((flags & !0x3f) as u32).unwrap();
    // 是否共享虚拟地址空间
    let new_memory_set = if clone_flags.contains(CloneFlags::CLONE_VM) {
        Arc::clone(&executor.memory_set)
    } else {
        let memory_set = Arc::new(Mutex::new(MemorySet::clone_or_err(
            &mut *executor.memory_set.lock().await,
        ).await?));

        {
            use async_axhal::mem::virt_to_phys;
            use async_axhal::paging::MappingFlags;
            // 生成信号跳板
            let signal_trampoline_vaddr: VirtAddr = (axconfig::SIGNAL_TRAMPOLINE).into();
            let signal_trampoline_paddr =
                virt_to_phys((start_signal_trampoline as usize).into());
            memory_set.lock().await.map_page_without_alloc(
                signal_trampoline_vaddr,
                signal_trampoline_paddr,
                MappingFlags::READ
                    | MappingFlags::EXECUTE
                    | MappingFlags::USER
                    | MappingFlags::WRITE,
            )?;
        }
        memory_set
    };

    // 在生成新的进程前，需要决定其所属进程是谁
    let process_id = if clone_flags.contains(CloneFlags::CLONE_THREAD) {
        // 当前clone生成的是线程，那么以self作为进程
        executor.pid
    } else {
        // 新建一个进程，并且设计进程之间的父子关系
        TaskId::new()
    };
    // 决定父进程是谁
    let parent_id = if clone_flags.contains(CloneFlags::CLONE_PARENT) {
        // 创建兄弟关系，此时以self的父进程作为自己的父进程
        // 理论上不应该创建内核进程的兄弟进程，所以可以直接unwrap
        executor.get_parent()
    } else {
        // 创建父子关系，此时以self作为父进程
        executor.pid.as_u64()
    };
    // let new_task = new_task(
    //     || {},
    //     String::from(executor.tasks.lock()[0].name().split('/').last().unwrap()),
    //     executor.get_stack_limit() as usize,
    //     process_id,
    //     new_memory_set.lock().await.page_table_token(),
    // );
    let scheduler = executor.get_scheduler();
    let fut = Box::pin(crate::user_task_top());
    let utrap_frame = *current_task().utrap_frame().unwrap();
    let new_task = Arc::new(Task::new(
        TaskInner::new_user(
            String::from(current_task().name().split('/').last().unwrap()),
            process_id.as_u64(),
            scheduler, 
            fut,
            utrap_frame,
        )
    ));

    // When clone a new task, the new task should have the same fs_base as the original task.
    //
    // It should be saved in trap frame, but to compatible with Unikernel, we save it in TaskContext.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        new_task.set_tls_force(async_axhal::arch::read_thread_pointer());
    }

    debug!("new task:{}", new_task.id_name());
    TID2TASK
        .lock()
        .await
        .insert(new_task.id().as_u64(), Arc::clone(&new_task));
    let new_handler = if clone_flags.contains(CloneFlags::CLONE_SIGHAND) {
        // let curr_id = current().id().as_u64();
        executor.signal_modules
            .lock()
            .await
            .get_mut(&current_task().id().as_u64())
            .unwrap()
            .signal_handler
            .clone()
    } else {
        Arc::new(Mutex::new(
            executor.signal_modules
                .lock()
                .await
                .get_mut(&current_task().id().as_u64())
                .unwrap()
                .signal_handler
                .lock()
                .await
                .clone(),
        ))
        // info!("curr_id: {:X}", (&curr_id as *const _ as usize));
    };
    // 检查是否在父任务中写入当前新任务的tid
    if clone_flags.contains(CloneFlags::CLONE_PARENT_SETTID)
        & executor.manual_alloc_for_lazy(ptid.into()).await.is_ok()
    {
        unsafe {
            *(ptid as *mut i32) = new_task.id().as_u64() as i32;
        }
    }
    // 若包含CLONE_CHILD_SETTID或者CLONE_CHILD_CLEARTID
    // 则需要把线程号写入到子线程地址空间中tid对应的地址中
    if clone_flags.contains(CloneFlags::CLONE_CHILD_SETTID)
        || clone_flags.contains(CloneFlags::CLONE_CHILD_CLEARTID)
    {
        if clone_flags.contains(CloneFlags::CLONE_CHILD_SETTID) {
            new_task.set_child_tid(ctid);
        }

        if clone_flags.contains(CloneFlags::CLONE_CHILD_CLEARTID) {
            new_task.set_clear_child_tid(ctid);
        }

        if clone_flags.contains(CloneFlags::CLONE_VM) {
            // 此时地址空间不会发生改变
            // 在当前地址空间下进行分配
            if executor.manual_alloc_for_lazy(ctid.into()).await.is_ok() {
                // 正常分配了地址
                unsafe {
                    *(ctid as *mut i32) =
                        if clone_flags.contains(CloneFlags::CLONE_CHILD_SETTID) {
                            new_task.id().as_u64() as i32
                        } else {
                            0
                        }
                }
            } else {
                return Err(AxError::BadAddress);
            }
        } else {
            // 否则需要在新的地址空间中进行分配
            let memory_set_wrapper = new_memory_set.lock().await;
            let mut vm = memory_set_wrapper;
            if vm.manual_alloc_for_lazy(ctid.into()).await.is_ok() {
                // 此时token没有发生改变，所以不能直接解引用访问，需要手动查页表
                if let Ok((phyaddr, _, _)) = vm.query(ctid.into()) {
                    let vaddr: usize = async_axhal::mem::phys_to_virt(phyaddr).into();
                    // 注意：任何地址都是从free memory分配来的，那么在页表中，free memory一直在页表中，他们的虚拟地址和物理地址一直有偏移的映射关系
                    unsafe {
                        *(vaddr as *mut i32) =
                            if clone_flags.contains(CloneFlags::CLONE_CHILD_SETTID) {
                                new_task.id().as_u64() as i32
                            } else {
                                0
                            }
                    }
                    drop(vm);
                } else {
                    drop(vm);
                    return Err(AxError::BadAddress);
                }
            } else {
                drop(vm);
                return Err(AxError::BadAddress);
            }
        }
    }
    // 返回的值
    // 若创建的是进程，则返回进程的id
    // 若创建的是线程，则返回线程的id
    let return_id: u64;
    // 决定是创建线程还是进程
    if clone_flags.contains(CloneFlags::CLONE_THREAD) {
        // // 若创建的是线程，那么不用新建进程
        // info!("task len: {}", inner.tasks.len());
        // info!("task address :{:X}", (&new_task as *const _ as usize));
        // info!(
        //     "task address: {:X}",
        //     (&Arc::clone(&new_task)) as *const _ as usize
        // );
        executor.get_scheduler().lock().add_task(Arc::clone(&new_task));

        let mut signal_module = SignalModule::init_signal(Some(new_handler));
        // exit signal, default to be SIGCHLD
        if exit_signal.is_some() {
            signal_module.set_exit_signal(exit_signal.unwrap());
        }
        executor.signal_modules
            .lock()
            .await
            .insert(new_task.id().as_u64(), signal_module);

        // self.robust_list
        //     .lock()
        //     .insert(new_task.id().as_u64(), FutexRobustList::default());
        return_id = new_task.id().as_u64();
    } else {
        let mut cwd_src = Arc::new(Mutex::new(String::from("/").into()));
        let mut mask_src = Arc::new(AtomicI32::new(0o022));
        if clone_flags.contains(CloneFlags::CLONE_FS) {
            cwd_src = Arc::clone(&executor.fd_manager.cwd);
            mask_src = Arc::clone(&executor.fd_manager.umask);
        }
        // 若创建的是进程，那么需要新建进程
        // 由于地址空间是复制的，所以堆底的地址也一定相同
        let fd_table = if clone_flags.contains(CloneFlags::CLONE_FILES) {
            Arc::clone(&executor.fd_manager.fd_table)
        } else {
            Arc::new(Mutex::new(executor.fd_manager.fd_table.lock().await.clone()))
        };
        let new_process = Arc::new(Executor::new(
            process_id, 
            parent_id, 
            new_memory_set, 
            executor.get_heap_bottom(), 
            fd_table, 
            cwd_src, 
            mask_src
        ));
        // 复制当前工作文件夹
        new_process.set_cwd(executor.get_cwd().await).await;
        // 记录该进程，防止被回收
        PID2PC.lock().await.insert(process_id.as_u64(), Arc::clone(&new_process));
        let scheduler = new_process.get_scheduler();
        new_task.set_scheduler(scheduler);
        new_process.put_prev_task(new_task.clone(), false);
        // new_process.tasks.lock().push(Arc::clone(&new_task));
        // 若是新建了进程，那么需要把进程的父子关系进行记录
        let mut signal_module = SignalModule::init_signal(Some(new_handler));
        // exit signal, default to be SIGCHLD
        if exit_signal.is_some() {
            signal_module.set_exit_signal(exit_signal.unwrap());
        }
        new_process
            .signal_modules
            .lock()
            .await
            .insert(new_task.id().as_u64(), signal_module);

        // new_process
        //     .robust_list
        //     .lock()
        //     .insert(new_task.id().as_u64(), FutexRobustList::default());
        return_id = new_process.pid.as_u64();
        PID2PC
            .lock()
            .await
            .get_mut(&parent_id)
            .unwrap()
            .children
            .lock()
            .await
            .push(Arc::clone(&new_process));

        let scheduler = executor.get_scheduler();
        let fut = Box::pin(new_process.run());
        let executor_task = Arc::new(Task::new(
            TaskInner::new(
                "executor".into(),
                process_id.as_u64(),
                scheduler, 
                fut,
            )
        ));
        executor.get_scheduler().lock().add_task(executor_task);
    };

    if !clone_flags.contains(CloneFlags::CLONE_THREAD) {
        new_task.set_leader(true);
    }
    // 复制原有的trap上下文
    // let mut trap_frame = unsafe { *(current_task.get_first_trap_frame()) };
    // let mut trap_frame =
    //     read_trapframe_from_kstack(current_task.get_kernel_stack_top().unwrap());
    // drop(current_task);
    // 新开的进程/线程返回值为0
    let utrap_frame = new_task.utrap_frame().unwrap();
    utrap_frame.set_ret_code(0);
    utrap_frame.trap_status = taskctx::TrapStatus::Done;
    // if clone_flags.contains(CloneFlags::CLONE_SETTLS) {
    //     #[cfg(not(target_arch = "x86_64"))]
    //     trap_frame.set_tls(tls);
    //     #[cfg(target_arch = "x86_64")]
    //     unsafe {
    //         new_task.set_tls_force(tls);
    //     }
    // }

    // 设置用户栈
    // 若给定了用户栈，则使用给定的用户栈
    // 若没有给定用户栈，则使用当前用户栈
    // 没有给定用户栈的时候，只能是共享了地址空间，且原先调用clone的有用户栈，此时已经在之前的trap clone时复制了
    if let Some(stack) = stack {
        utrap_frame.set_user_sp(stack);
        // info!(
        //     "New user stack: sepc:{:X}, stack:{:X}",
        //     trap_frame.sepc, trap_frame.regs.sp
        // );
    }
    #[cfg(feature = "future")]
    {
        let stack_size = self.get_stack_limit() as usize;
        new_task.init_user_kstack(stack_size, core::mem::size_of::<TrapFrame>());
        new_task.set_ctx_type(axtask::ContextType::THREAD);
    }
    // write_trapframe_to_kstack(new_task.get_kernel_stack_top().unwrap(), &trap_frame);
    // Processor::first_add_task(new_task);
    // 判断是否为VFORK
    if clone_flags.contains(CloneFlags::CLONE_VFORK) {
        executor.set_vfork_block(true).await;
        // VFORK: TODO: judge when schedule
        while executor.get_vfork_block().await {
            executor::yield_now().await;
        }
    }
    Ok(return_id)
}

/// 将当前进程替换为指定的用户程序
/// args为传入的参数
/// 任务的统计时间会被重置
pub async fn exec(
    executor: &Arc<Executor>, 
    name: String, 
    args: Vec<String>, 
    envs: &Vec<String>
) -> AxResult<()> {
    // 首先要处理原先进程的资源
    // 处理分配的页帧
    // 之后加入额外的东西之后再处理其他的包括信号等因素
    // 不是直接删除原有地址空间，否则构建成本较高。

    if Arc::strong_count(&executor.memory_set) == 1 {
        executor.memory_set.lock().await.unmap_user_areas();
    } else {
        let memory_set = MemorySet::clone_or_err(
            &mut *executor.memory_set.lock().await,
        ).await?;
        *executor.memory_set.lock().await = memory_set;
        executor.memory_set.lock().await.unmap_user_areas();
        let new_page_table = executor.memory_set.lock().await.page_table_token();
        // let mut tasks = self.tasks.lock();
        // for task in tasks.iter_mut() {
        //     task.inner().set_page_table_token(new_page_table);
        // }
        // 切换到新的页表上
        unsafe {
            async_axhal::arch::write_page_table_root0(new_page_table.into());
        }
    }
    // 清空用户堆，重置堆顶
    async_axhal::arch::flush_tlb(None);

    // 关闭 `CLOEXEC` 的文件
    executor.fd_manager.close_on_exec().await;
    let current_task = current_task();
    // 再考虑手动结束其他所有的task
    // 暂时不支持其他任务，因此直接注释掉下面的代码，并且目前的设计也不支持寻找其他的任务
    // let mut tasks = self.tasks.lock();
    // for _ in 0..tasks.len() {
    //     let task = tasks.pop().unwrap();
    //     if task.id() == current_task.id() {
    //         // FIXME: This will reset tls forcefully
    //         #[cfg(target_arch = "x86_64")]
    //         unsafe {
    //             task.set_tls_force(0);
    //             axhal::arch::write_thread_pointer(0);
    //         }
    //         tasks.push(task);
    //     } else {
    //         TID2TASK.lock().remove(&task.id().as_u64());
    //         panic!("currently not support exec when has another task ");
    //     }
    // }
    // 当前任务被设置为主线程
    current_task.set_leader(true);
    // 重置统计时间
    current_task.time_stat_reset(current_time_nanos() as usize);
    current_task.set_name(name.split('/').last().unwrap());
    // assert!(tasks.len() == 1);
    // drop(tasks);
    let args = if args.is_empty() {
        vec![name.clone()]
    } else {
        args
    };
    let (entry, user_stack_bottom, heap_bottom) = if let Ok(ans) =
        load_app(name.clone(), args, envs, &mut *executor.memory_set.lock().await).await
    {
        ans
    } else {
        error!("Failed to load app {}", name);
        return Err(AxError::NotFound);
    };
    // 切换了地址空间， 需要切换token
    let page_table_token = if executor.pid.as_u64() == KERNEL_EXECUTOR_ID {
        0
    } else {
        executor.memory_set.lock().await.page_table_token()
    };
    if page_table_token != 0 {
        unsafe {
            async_axhal::arch::write_page_table_root0(page_table_token.into());
        };
        // 清空用户堆，重置堆顶
    }
    // 重置用户堆
    executor.set_heap_bottom(heap_bottom.as_usize() as u64);
    executor.set_heap_top(heap_bottom.as_usize() as u64);
    // // // 重置robust list
    // self.robust_list.lock().clear();
    // self.robust_list
    //     .lock()
    //     .insert(current_task.id().as_u64(), FutexRobustList::default());

    {
        use async_axhal::mem::virt_to_phys;
        use async_axhal::paging::MappingFlags;
        // 重置信号处理模块
        // 此时只会留下一个线程
        executor.signal_modules.lock().await.clear();
        executor.signal_modules
            .lock()
            .await
            .insert(current_task.id().as_u64(), SignalModule::init_signal(None));

        // 生成信号跳板
        let signal_trampoline_vaddr: VirtAddr = (axconfig::SIGNAL_TRAMPOLINE).into();
        let signal_trampoline_paddr = virt_to_phys((start_signal_trampoline as usize).into());
        let memory_set_wrapper = executor.memory_set.lock();
        let mut memory_set = memory_set_wrapper.await;
        if memory_set.query(signal_trampoline_vaddr).is_err() {
            let _ = memory_set.map_page_without_alloc(
                signal_trampoline_vaddr,
                signal_trampoline_paddr,
                MappingFlags::READ
                    | MappingFlags::EXECUTE
                    | MappingFlags::USER
                    | MappingFlags::WRITE,
            );
        }
        drop(memory_set);
    }

    // // user_stack_top = user_stack_top / PAGE_SIZE_4K * PAGE_SIZE_4K;
    // let new_trap_frame =
    //     TrapFrame::app_init_context(entry.as_usize(), user_stack_bottom.as_usize());
    // write_trapframe_to_kstack(
    //     current_task.get_kernel_stack_top().unwrap(),
    //     &new_trap_frame,
    // );
    let utrap_frame = current_task.utrap_frame().unwrap();
    *utrap_frame = TrapFrame::init_user_context(entry.as_usize(), user_stack_bottom.as_usize());

    // release vfork for parent process
    {
        let pid2pc = PID2PC.lock().await;
        let parent_process = pid2pc.get(&executor.get_parent()).unwrap();
        parent_process.set_vfork_block(false).await;
        drop(pid2pc);
    }
    Ok(())
}