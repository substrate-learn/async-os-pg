// #![no_std]

// extern crate alloc;

// #[macro_use]
// extern crate axlog;
// // mod executor;
// #[allow(unused)]
// mod waker;

// // pub use executor::Executor;

// mod link;
// // mod fd_manager;
// // mod stdio;
// use core::{ptr::copy_nonoverlapping, str::from_utf8};

// use axconfig::{MAX_USER_HEAP_SIZE, MAX_USER_STACK_SIZE, USER_HEAP_BASE, USER_STACK_TOP};
// use axhal::{mem::VirtAddr, paging::MappingFlags};
// use elf_parser::{get_app_stack_region, get_auxv_vector, get_elf_entry, get_elf_segments, get_relocate_pairs};
// pub use link::*;

// use alloc::{boxed::Box, string::{String, ToString}, vec::Vec};
// use async_axtask::AxTaskRef;
// use axerrno::{AxError, AxResult};
// use axmem::MemorySet;
// use xmas_elf::program::SegmentData;
// use alloc::vec;

// extern "C" {
//     fn start_signal_trampoline();
// }

// /// 返回应用程序入口，用户栈底，用户堆底
// pub async fn load_app(
//     name: String,
//     mut args: Vec<String>,
//     envs: &Vec<String>,
//     memory_set: &mut MemorySet,
// ) -> AxResult<(VirtAddr, VirtAddr, VirtAddr)> {
//     if name.ends_with(".sh") {
//         args = [vec![String::from("busybox"), String::from("sh")], args].concat();
//         return Box::pin(load_app("busybox".to_string(), args, envs, memory_set)).await;
//     }
//     let elf_data = if let Ok(ans) = axfs::api::read(name.as_str()).await {
//         ans
//     } else {
//         // exit(0)
//         info!("App not found: {}", name);
//         return Err(AxError::NotFound);
//     };
//     let elf = xmas_elf::ElfFile::new(&elf_data).expect("Error parsing app ELF file.");
//     if let Some(interp) = elf
//         .program_iter()
//         .find(|ph| ph.get_type() == Ok(xmas_elf::program::Type::Interp))
//     {
//         let interp = match interp.get_data(&elf) {
//             Ok(SegmentData::Undefined(data)) => data,
//             _ => panic!("Invalid data in Interp Elf Program Header"),
//         };

//         let interp_path = from_utf8(interp).expect("Interpreter path isn't valid UTF-8");
//         // remove trailing '\0'
//         let interp_path = interp_path.trim_matches(char::from(0)).to_string();
//         let real_interp_path = real_path(&interp_path).await;
//         args = [vec![real_interp_path.clone()], args].concat();
//         return Box::pin(load_app(real_interp_path, args, envs, memory_set)).await;
//     }
//     info!("load app args: {:?} name: {}", args, name);
//     let elf_base_addr = Some(0x400_0000);
//     warn!("The elf base addr may be different in different arch!");
//     // let (entry, segments, relocate_pairs) = parse_elf(&elf, elf_base_addr);
//     let entry = get_elf_entry(&elf, elf_base_addr);
//     let segments = get_elf_segments(&elf, elf_base_addr);
//     let relocate_pairs = get_relocate_pairs(&elf, elf_base_addr);
//     for segment in segments {
//         memory_set.new_region(
//             segment.vaddr,
//             segment.size,
//             false,
//             segment.flags,
//             segment.data.as_deref(),
//             None,
//         ).await;
//     }

//     for relocate_pair in relocate_pairs {
//         let src: usize = relocate_pair.src.into();
//         let dst: usize = relocate_pair.dst.into();
//         let count = relocate_pair.count;
//         unsafe { copy_nonoverlapping(src.to_ne_bytes().as_ptr(), dst as *mut u8, count) }
//     }

//     // Now map the stack and the heap
//     let heap_start = VirtAddr::from(USER_HEAP_BASE);
//     let heap_data = [0_u8].repeat(MAX_USER_HEAP_SIZE);
//     memory_set.new_region(
//         heap_start,
//         MAX_USER_HEAP_SIZE,
//         false,
//         MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
//         Some(&heap_data),
//         None,
//     ).await;
//     info!(
//         "[new region] user heap: [{:?}, {:?})",
//         heap_start,
//         heap_start + MAX_USER_HEAP_SIZE
//     );

//     let auxv = get_auxv_vector(&elf, elf_base_addr);

//     let stack_top = VirtAddr::from(USER_STACK_TOP);
//     let stack_size = MAX_USER_STACK_SIZE;

//     let (stack_data, stack_bottom) = get_app_stack_region(args, envs, auxv, stack_top, stack_size);
//     memory_set.new_region(
//         stack_top,
//         stack_size,
//         false,
//         MappingFlags::USER | MappingFlags::READ | MappingFlags::WRITE,
//         Some(&stack_data),
//         None,
//     ).await;
//     info!(
//         "[new region] user stack: [{:?}, {:?})",
//         stack_top,
//         stack_top + stack_size
//     );
//     Ok((entry, stack_bottom.into(), heap_start))
// }

// /// 根据给定参数创建一个新的进程，作为应用程序初始进程
// pub async fn init_user(args: Vec<String>, envs: &Vec<String>) -> AxResult<AxTaskRef> {
//     let path = args[0].clone();
//     let mut memory_set = MemorySet::new_memory_set();

//     {
//         use axhal::mem::virt_to_phys;
//         use axhal::paging::MappingFlags;
//         // 生成信号跳板
//         let signal_trampoline_vaddr: VirtAddr = (axconfig::SIGNAL_TRAMPOLINE).into();
//         let signal_trampoline_paddr = virt_to_phys((start_signal_trampoline as usize).into());
//         memory_set.map_page_without_alloc(
//             signal_trampoline_vaddr,
//             signal_trampoline_paddr,
//             MappingFlags::READ
//                 | MappingFlags::EXECUTE
//                 | MappingFlags::USER
//                 | MappingFlags::WRITE,
//         )?;
//     }
//     let page_table_token = memory_set.page_table_token();
//     if page_table_token != 0 {
//         unsafe {
//             axhal::arch::write_page_table_root0(page_table_token.into());
//             #[cfg(target_arch = "riscv64")]
//             riscv::register::sstatus::set_sum();
//         };
//     }

//     let (_entry, _user_stack_bottom, _heap_bottom) =
//         if let Ok(ans) = load_app(path.clone(), args, envs, &mut memory_set).await {
//             ans
//         } else {
//             error!("Failed to load app {}", path);
//             return Err(AxError::NotFound);
//         };
//     Err(AxError::NotFound)
//     // let new_fd_table: FdTable = Arc::new(Mutex::new(vec![
//     //     // 标准输入
//     //     Some(Arc::new(Stdin {
//     //         flags: Mutex::new(OpenFlags::empty()),
//     //     })),
//     //     // 标准输出
//     //     Some(Arc::new(Stdout {
//     //         flags: Mutex::new(OpenFlags::empty()),
//     //     })),
//     //     // 标准错误
//     //     Some(Arc::new(Stderr {
//     //         flags: Mutex::new(OpenFlags::empty()),
//     //     })),
//     // ]));
//     // let new_process = Arc::new(Self::new(
//     //     TaskId::new().as_u64(),
//     //     axconfig::TASK_STACK_SIZE as u64,
//     //     KERNEL_PROCESS_ID,
//     //     Mutex::new(Arc::new(Mutex::new(memory_set))),
//     //     heap_bottom.as_usize() as u64,
//     //     Arc::new(Mutex::new(String::from("/").into())),
//     //     Arc::new(AtomicI32::new(0o022)),
//     //     new_fd_table,
//     // ));
//     // if !path.starts_with('/') {
//     //     //如果path不是绝对路径, 则加上当前工作目录
//     //     let cwd = new_process.get_cwd();
//     //     assert!(cwd.ends_with('/'));
//     //     path = format!("{}{}", cwd, path);
//     // }
//     // new_process.set_file_path(path.clone());
//     // let new_task = new_task(
//     //     || {},
//     //     path,
//     //     new_process.get_stack_limit() as usize,
//     //     new_process.pid(),
//     //     page_table_token,
//     // );
//     // TID2TASK
//     //     .lock()
//     //     .insert(new_task.id().as_u64(), Arc::clone(&new_task));
//     // new_task.set_leader(true);
//     // #[cfg(feature = "future")]
//     // {
//     //     let stack_size = new_process.get_stack_limit() as usize;
//     //     new_task.init_user_kstack(stack_size, core::mem::size_of::<TrapFrame>());
//     //     new_task.set_ctx_type(axtask::ContextType::THREAD);
//     // }
//     // let new_trap_frame =
//     //     TrapFrame::app_init_context(entry.as_usize(), user_stack_bottom.as_usize());
//     // // // 需要将完整内容写入到内核栈上，first_into_user并不会复制到内核栈上
//     // write_trapframe_to_kstack(new_task.get_kernel_stack_top().unwrap(), &new_trap_frame);
//     // new_process.tasks.lock().push(Arc::clone(&new_task));

//     // new_process
//     //     .signal_modules
//     //     .lock()
//     //     .insert(new_task.id().as_u64(), SignalModule::init_signal(None));
//     // new_process
//     //     .robust_list
//     //     .lock()
//     //     .insert(new_task.id().as_u64(), FutexRobustList::default());
//     // PID2PC
//     //     .lock()
//     //     .insert(new_process.pid(), Arc::clone(&new_process));
//     // // 将其作为内核进程的子进程
//     // match PID2PC.lock().get(&KERNEL_PROCESS_ID) {
//     //     Some(kernel_process) => {
//     //         kernel_process.children.lock().push(new_process);
//     //     }
//     //     None => {
//     //         return Err(AxError::NotFound);
//     //     }
//     // }
//     // Processor::first_add_task(Arc::clone(&new_task));
//     // Ok(new_task)
// }

// pub async fn fs_init() {
//     use alloc::format;
//     use alloc::string::ToString;
//     #[cfg(target_arch = "riscv64")]
//     let libc_so = &"ld-musl-riscv64-sf.so.1";
//     #[cfg(target_arch = "riscv64")]
//     let libc_so2 = &"ld-musl-riscv64.so.1"; // 另一种名字的 libc.so，非 libc-test 测例库用

//     #[cfg(target_arch = "x86_64")]
//     let libc_so = &"ld-musl-x86_64-sf.so.1";
//     #[cfg(target_arch = "x86_64")]
//     let libc_so2 = &"ld-musl-x86_64.so.1"; // 另一种名字的 libc.so，非 libc-test 测例库用

//     #[cfg(target_arch = "aarch64")]
//     let libc_so = &"ld-musl-aarch64-sf.so.1";
//     #[cfg(target_arch = "aarch64")]
//     let libc_so2 = &"ld-musl-aarch64.so.1"; // 另一种名字的 libc.so，非 libc-test 测例库用

//     create_link(
//         &(FilePath::new(("/lib/".to_string() + libc_so).as_str()).await.unwrap()),
//         &(FilePath::new("libc.so").await.unwrap()),
//     ).await;
//     create_link(
//         &(FilePath::new(("/lib/".to_string() + libc_so2).as_str()).await.unwrap()),
//         &(FilePath::new("libc.so").await.unwrap()),
//     ).await;

//     let tls_so = &"tls_get_new-dtv_dso.so";
//     create_link(
//         &(FilePath::new(("/lib/".to_string() + tls_so).as_str()).await.unwrap()),
//         &(FilePath::new("tls_get_new-dtv_dso.so").await.unwrap()),
//     ).await;

//     // 接下来对 busybox 相关的指令建立软链接
//     let busybox_arch = ["ls", "mkdir", "touch", "mv", "busybox", "sh", "which", "cp"];
//     for arch in busybox_arch {
//         let src_path = "/usr/sbin/".to_string() + arch;
//         create_link(
//             &(FilePath::new(src_path.as_str()).await.unwrap()),
//             &(FilePath::new("busybox").await.unwrap()),
//         ).await;
//         let src_path = "/usr/bin/".to_string() + arch;
//         create_link(
//             &(FilePath::new(src_path.as_str()).await.unwrap()),
//             &(FilePath::new("busybox").await.unwrap()),
//         ).await;
//         let src_path = "/bin/".to_string() + arch;
//         create_link(
//             &(FilePath::new(src_path.as_str()).await.unwrap()),
//             &(FilePath::new("busybox").await.unwrap()),
//         ).await;
//     }
//     create_link(
//         &(FilePath::new("/bin/lmbench_all").await.unwrap()),
//         &(FilePath::new("/lmbench_all").await.unwrap()),
//     ).await;
//     create_link(
//         &(FilePath::new("/bin/iozone").await.unwrap()),
//         &(FilePath::new("/iozone").await.unwrap()),
//     ).await;

//     #[cfg(target_arch = "x86_64")]
//     {
//         let libc_zlm = &"/lib/ld-linux-x86-64.so.2";
//         create_link(
//             &(FilePath::new(libc_zlm).await.unwrap()),
//             &(FilePath::new("ld-linux-x86-64.so.2").await.unwrap()),
//         ).await;

//         create_link(
//             &(FilePath::new("/lib/libssl.so.3").await.unwrap()),
//             &(FilePath::new("libssl.so.3").await.unwrap()),
//         ).await;

//         create_link(
//             &(FilePath::new("/lib/libcrypto.so.3").await.unwrap()),
//             &(FilePath::new("libcrypto.so.3").await.unwrap()),
//         ).await;

//         create_link(
//             &(FilePath::new("/lib/libstdc++.so.6").await.unwrap()),
//             &(FilePath::new("libstdc++.so.6").await.unwrap()),
//         ).await;

//         create_link(
//             &(FilePath::new("/lib/libm.so.6").await.unwrap()),
//             &(FilePath::new("libm.so.6").await.unwrap()),
//         ).await;

//         create_link(
//             &(FilePath::new("/lib/libgcc_s.so.1").await.unwrap()),
//             &(FilePath::new("libgcc_s.so.1").await.unwrap()),
//         ).await;

//         create_link(
//             &(FilePath::new("/lib/libc.so.6").await.unwrap()),
//             &(FilePath::new("libc.so.6").await.unwrap()),
//         ).await;
//     }

//     // let mem_file = axfs::api::lookup("/proc/meminfo").await.unwrap();
//     // mem_file.write_at(0, meminfo().as_bytes()).unwrap();
//     // let oom_file = axfs::api::lookup("/proc/sys/vm/overcommit_memory").await.unwrap();
//     // oom_file.write_at(0, oominfo().as_bytes()).unwrap();
//     // let fs_file = axfs::api::lookup("/proc/filesystems").await.unwrap();
//     // fs_file.write_at(0, b"fat32\next4\n").unwrap();

//     // let status_file = axfs::api::lookup("/proc/self/status").await.unwrap();
//     // let status_info: &[u8] = &get_status_info(&axtask::current());
//     // status_file.write_at(0, status_info).unwrap();

//     // // create the file for the lmbench testcase
//     // let _ = new_file("/lat_sig", &(FileFlags::CREATE | FileFlags::RDWR));

//     // gcc相关的链接，可以在testcases/gcc/riscv64-linux-musl-native/lib目录下使用ls -al指令查看
//     let src_dir = "riscv64-linux-musl-native/lib";
//     create_link(
//         &FilePath::new(format!("{}/ld-musl-riscv64.so.1", src_dir).as_str()).await.unwrap(),
//         &FilePath::new("/lib/libc.so").await.unwrap(),
//     ).await;
//     create_link(
//         &FilePath::new(format!("{}/libatomic.so", src_dir).as_str()).await.unwrap(),
//         &FilePath::new(format!("{}/libatomic.so.1.2.0", src_dir).as_str()).await.unwrap(),
//     ).await;
//     create_link(
//         &FilePath::new(format!("{}/libatomic.so.1", src_dir).as_str()).await.unwrap(),
//         &FilePath::new(format!("{}/libatomic.so.1.2.0", src_dir).as_str()).await.unwrap(),
//     ).await;
//     create_link(
//         &FilePath::new(format!("{}/libgfortran.so", src_dir).as_str()).await.unwrap(),
//         &FilePath::new(format!("{}/libgfortran.so.5.0.0", src_dir).as_str()).await.unwrap(),
//     ).await;
//     create_link(
//         &FilePath::new(format!("{}/libgfortran.so.5", src_dir).as_str()).await.unwrap(),
//         &FilePath::new(format!("{}/libgfortran.so.5.0.0", src_dir).as_str()).await.unwrap(),
//     ).await;
//     create_link(
//         &FilePath::new(format!("{}/libgomp.so", src_dir).as_str()).await.unwrap(),
//         &FilePath::new(format!("{}/libgomp.so.1.0.0", src_dir).as_str()).await.unwrap(),
//     ).await;
//     create_link(
//         &FilePath::new(format!("{}/libgomp.so.1", src_dir).as_str()).await.unwrap(),
//         &FilePath::new(format!("{}/libgomp.so.1.0.0", src_dir).as_str()).await.unwrap(),
//     ).await;
//     create_link(
//         &FilePath::new(format!("{}/libssp.so", src_dir).as_str()).await.unwrap(),
//         &FilePath::new(format!("{}/libssp.so.0.0.0", src_dir).as_str()).await.unwrap(),
//     ).await;
//     create_link(
//         &FilePath::new(format!("{}/libssp.so.0", src_dir).as_str()).await.unwrap(),
//         &FilePath::new(format!("{}/libssp.so.0.0.0", src_dir).as_str()).await.unwrap(),
//     ).await;
//     create_link(
//         &FilePath::new(format!("{}/libstdc++.so", src_dir).as_str()).await.unwrap(),
//         &FilePath::new(format!("{}/libstdc++.so.6.0.29", src_dir).as_str()).await.unwrap(),
//     ).await;
//     create_link(
//         &FilePath::new(format!("{}/libstdc++.so.6", src_dir).as_str()).await.unwrap(),
//         &FilePath::new(format!("{}/libstdc++.so.6.0.29", src_dir).as_str()).await.unwrap(),
//     ).await;
// }


// // fn meminfo() -> &'static str {
// //     "MemTotal:       32246488 kB
// // MemFree:         5239804 kB
// // MemAvailable:   10106000 kB
// // Buffers:          235604 kB
// // Cached:          5204940 kB
// // SwapCached:            0 kB
// // Active:         17890456 kB
// // Inactive:        2119348 kB
// // Active(anon):   14891328 kB
// // Inactive(anon):        0 kB
// // Active(file):    2999128 kB
// // Inactive(file):  2119348 kB
// // Unevictable:         144 kB
// // Mlocked:             144 kB
// // SwapTotal:       8388604 kB
// // SwapFree:        8388604 kB
// // Zswap:                 0 kB
// // Zswapped:              0 kB
// // Dirty:               784 kB
// // Writeback:             0 kB
// // AnonPages:      14560300 kB
// // Mapped:          2108592 kB
// // Shmem:            323608 kB
// // KReclaimable:     205804 kB
// // Slab:            1539752 kB
// // SReclaimable:     205804 kB
// // SUnreclaim:      1333948 kB
// // KernelStack:      630704 kB
// // PageTables:      2007248 kB
// // SecPageTables:         0 kB
// // NFS_Unstable:          0 kB
// // Bounce:                0 kB
// // WritebackTmp:          0 kB
// // CommitLimit:    24511848 kB
// // Committed_AS:   42466972 kB
// // VmallocTotal:   34359738367 kB
// // VmallocUsed:      762644 kB
// // VmallocChunk:          0 kB
// // Percpu:            35776 kB
// // HardwareCorrupted:     0 kB
// // AnonHugePages:     79872 kB
// // ShmemHugePages:        0 kB
// // ShmemPmdMapped:        0 kB
// // FileHugePages:         0 kB
// // FilePmdMapped:         0 kB
// // Unaccepted:            0 kB
// // HugePages_Total:       0
// // HugePages_Free:        0
// // HugePages_Rsvd:        0
// // HugePages_Surp:        0
// // Hugepagesize:       2048 kB
// // Hugetlb:               0 kB
// // DirectMap4k:     6500036 kB
// // DirectMap2M:    23283712 kB
// // DirectMap1G:     3145728 kB"
// // }

// // // TODO: Implement the real content of overcommit_memory
// // fn oominfo() -> &'static str {
// //     "0"
// // }

// // fn get_status_info(task: &axtask::CurrentTask) -> Vec<u8> {
// //     let name = task.name().as_bytes();
// //     let id = task.id().as_u64().to_string();
// //     let status_vec = [name, b"\n", id.as_bytes(), b"\n256\n"].concat();
// //     status_vec
// // }