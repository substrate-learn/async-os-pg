use core::sync::atomic::AtomicI32;

use alloc::{boxed::Box, format, string::String, sync::Arc, vec, vec::Vec};
use async_mem::MemorySet;
use axerrno::{AxError, AxResult};
use executor::{load_app, Executor, FdTable, Stderr, Stdin, Stdout, KERNEL_EXECUTOR_ID, PID2PC, TID2TASK};
use sync::Mutex;
use taskctx::{BaseScheduler, Task, TaskId, TaskInner, TaskRef, TrapFrame};
use async_axhal::mem::VirtAddr;
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
    let new_task = Arc::new(Task::new(
        TaskInner::new_user(
            path, 
            scheduler, 
            fut,
            TrapFrame::init_user_context(
                entry.into(), user_stack_bottom.into()
            )
        )
    ));
    if new_task.utrap_frame().is_none() {
        panic!("new_task utrap_frame empty");
    }
    warn!("new_task {}, count {}", new_task.id_name(), Arc::strong_count(&new_task));

    // let new_task = spawn_raw(|| run_user_task(entry), path);
    // let new_task = new_task(Box::pin(UserTask::new(entry, user_stack_bottom)), path);
    // Executor::add_task(new_task.clone());
    new_executor.get_scheduler().lock().add_task(new_task.clone());
    warn!("new_task {}, count {}", new_task.id_name(), Arc::strong_count(&new_task));
    TID2TASK
        .lock().await
        .insert(new_task.id().as_u64(), Arc::clone(&new_task));
    // new_task.set_leader(true);
    warn!("new_task {}, count {}", new_task.id_name(), Arc::strong_count(&new_task));

    // new_process
    //     .signal_modules
    //     .lock()
    //     .insert(new_task.id().as_u64(), SignalModule::init_signal(None));
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
    warn!("new_task {}, count {}", new_task.id_name(), Arc::strong_count(&new_task));

    Ok(new_task)
}