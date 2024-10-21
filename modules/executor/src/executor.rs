
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use alloc::{collections::btree_map::BTreeMap, string::{String, ToString}, sync::Arc, vec::Vec, vec};
use axerrno::{AxError, AxResult};
use async_fs::api::{FileIO, OpenFlags};
use async_mem::MemorySet;
use axhal::mem::VirtAddr;
use taskctx::{BaseScheduler, TaskRef};
use spinlock::SpinNoIrq;
use sync::Mutex;
use taskctx::{Scheduler, TaskId};
use crate::{fd_manager::{FdManager, FdTable}, stdio::{Stderr, Stdin, Stdout}};

const FD_LIMIT_ORIGIN: usize = 1025;
pub const KERNEL_EXECUTOR_ID: u64 = 1;
pub static TID2TASK: Mutex<BTreeMap<u64, TaskRef>> = Mutex::new(BTreeMap::new());
pub static PID2PC: Mutex<BTreeMap<u64, Arc<Executor>>> = Mutex::new(BTreeMap::new());


pub struct Executor {
    pub pid: TaskId,
    pub parent: AtomicU64,
    /// 子进程
    pub children: Mutex<Vec<Arc<Executor>>>,
    scheduler: Arc<SpinNoIrq<Scheduler>>,
    /// 文件描述符管理器
    pub fd_manager: FdManager,
    /// 进程状态
    pub is_zombie: AtomicBool,
    /// 退出状态码
    pub exit_code: AtomicI32,

    /// 地址空间
    pub memory_set: Arc<Mutex<MemorySet>>,
    /// 用户堆基址，任何时候堆顶都不能比这个值小，理论上讲是一个常量
    pub heap_bottom: AtomicU64,
    /// 当前用户堆的堆顶，不能小于基址，不能大于基址加堆的最大大小
    pub heap_top: AtomicU64,
    /// 是否被vfork阻塞
    pub blocked_by_vfork: Mutex<bool>,
    /// 该进程可执行文件所在的路径
    pub file_path: Mutex<String>,
}

unsafe impl Sync for Executor {}
unsafe impl Send for Executor {}

impl Executor {
    
    /// 创建一个新的 Executor（进程）
    pub fn new(
        pid: TaskId,
        parent: u64,
        memory_set: Arc<Mutex<MemorySet>>,
        heap_bottom: u64,
        fd_table: FdTable,
        cwd: Arc<Mutex<String>>,
        mask: Arc<AtomicI32>,
    ) -> Self {
        let mut scheduler = Scheduler::new();
        scheduler.init();
        Self {
            pid,
            parent: AtomicU64::new(parent),
            children: Mutex::new(Vec::new()),
            scheduler: Arc::new(SpinNoIrq::new(scheduler)),
            fd_manager: FdManager::new(fd_table, cwd, mask, FD_LIMIT_ORIGIN),
            is_zombie: AtomicBool::new(false),
            exit_code: AtomicI32::new(0),
            memory_set,
            heap_bottom: AtomicU64::new(heap_bottom),
            heap_top: AtomicU64::new(heap_bottom),
            blocked_by_vfork: Mutex::new(false),
            file_path: Mutex::new(String::new()),
        }
    }

    /// 内核 Executor
    pub fn new_init() -> Self {
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
        Executor::new(
            TaskId::new(), 
            KERNEL_EXECUTOR_ID, 
            Arc::new(Mutex::new(MemorySet::new_memory_set())),
            0,
            new_fd_table,
            Arc::new(Mutex::new(String::from("/"))),
            Arc::new(AtomicI32::new(0o022)),
        )
    }

    /// 获取调度器
    pub fn get_scheduler(&self) -> Arc<SpinNoIrq<Scheduler>> {
        self.scheduler.clone()
    }

    /// 获取 Executor（进程）id
    pub fn pid(&self) -> TaskId {
        self.pid
    }

    /// 获取父 Executor（进程）id
    pub fn get_parent(&self) -> u64 {
        self.parent.load(Ordering::Acquire)
    }

    /// 设置父 Executor（进程）id
    pub fn set_parent(&self, parent: u64) {
        self.parent.store(parent, Ordering::Release)
    }

    /// 获取 Executor（进程）退出码
    pub fn get_exit_code(&self) -> i32 {
        self.exit_code.load(Ordering::Acquire)
    }

    /// 设置 Executor（进程）退出码
    pub fn set_exit_code(&self, exit_code: i32) {
        self.exit_code.store(exit_code, Ordering::Release)
    }

    /// 判断 Executor（进程）是否处于僵尸状态
    pub fn get_zombie(&self) -> bool {
        self.is_zombie.load(Ordering::Acquire)
    }

    /// 设置 Executor（进程）是否处于僵尸状态
    pub fn set_zombie(&self, status: bool) {
        self.is_zombie.store(status, Ordering::Release)
    }

    /// 获取 Executor（进程）的堆顶
    pub fn get_heap_top(&self) -> u64 {
        self.heap_top.load(Ordering::Acquire)
    }

    /// 设置 Executor（进程）的堆顶
    pub fn set_heap_top(&self, top: u64) {
        self.heap_top.store(top, Ordering::Release)
    }

    /// 获取 Executor（进程）的堆底
    pub fn get_heap_bottom(&self) -> u64 {
        self.heap_bottom.load(Ordering::Acquire)
    }

    /// 设置 Executor（进程）的堆底
    pub fn set_heap_bottom(&self, bottom: u64) {
        self.heap_bottom.store(bottom, Ordering::Release)
    }

    /// 设置 Executor（进程）是否被 vfork 阻塞
    pub async fn set_vfork_block(&self, value: bool) {
        *self.blocked_by_vfork.lock().await = value;
    }

    /// 获取 Executor（进程）是否被 vfork 阻塞
    pub async fn get_vfork_block(&self) -> bool {
        *self.blocked_by_vfork.lock().await
    }

    /// 设置 Executor（进程）可执行文件路径
    pub async fn set_file_path(&self, path: String) {
        let mut file_path = self.file_path.lock().await;
        *file_path = path;
    }

    /// 获取 Executor（进程）可执行文件路径
    pub async fn get_file_path(&self) -> String {
        (*self.file_path.lock().await).clone()
    }

    /// 若进程运行完成，则获取其返回码
    /// 若正在运行（可能上锁或没有上锁），则返回None
    pub fn get_code_if_exit(&self) -> Option<i32> {
        if self.get_zombie() {
            return Some(self.get_exit_code());
        }
        None
    }

    #[inline]
    /// Pick one task from Executor
    pub fn pick_next_task(&self) -> Option<TaskRef> {
        self.scheduler
            .lock()
            .pick_next_task()
    }

    #[inline]
    /// Add curr task to Executor, it ususally add to back
    pub fn put_prev_task(&self, task: TaskRef, front: bool) {
        self.scheduler.lock().put_prev_task(task, front);
    }

    #[inline]
    /// Add task to Executor, now just put it to own Executor
    /// TODO: support task migrate on differ Executor
    pub fn add_task(task: TaskRef) {
        task.get_scheduler().lock().add_task(task);
    }

    #[inline]
    /// Executor Clean
    pub fn task_tick(&self, task: &TaskRef) -> bool {
        self.scheduler.lock().task_tick(task)
    }

    #[inline]
    /// Executor Clean
    pub fn set_priority(&self, task: &TaskRef, prio: isize) -> bool {
        self.scheduler.lock().set_priority(task, prio)
    }

    #[inline]
    pub async fn run(self: Arc<Self>) -> i32 {
        crate::CurrentExecutor::clean_current();
        unsafe { crate::CurrentExecutor::init_current(self.clone()) };
        let page_table_token = self.memory_set.lock().await.page_table_token();
        if page_table_token != 0 {
            unsafe {
                axhal::arch::write_page_table_root0(page_table_token.into());
                #[cfg(target_arch = "riscv64")]
                riscv::register::sstatus::set_sum();
                axhal::arch::flush_tlb(None);
            };
        }
        0
    }

}

impl Executor {

    /// 获取当前进程的工作目录
    pub async fn get_cwd(&self) -> String {
        self.fd_manager.cwd.lock().await.clone().to_string()
    }

    /// Set the current working directory of the process
    pub async fn set_cwd(&self, cwd: String) {
        *self.fd_manager.cwd.lock().await = cwd.into();
    }

    /// alloc physical memory for lazy allocation manually
    pub async fn manual_alloc_for_lazy(&self, addr: VirtAddr) -> AxResult<()> {
        self.memory_set.lock().await.manual_alloc_for_lazy(addr).await
    }

    /// alloc range physical memory for lazy allocation manually
    pub async fn manual_alloc_range_for_lazy(&self, start: VirtAddr, end: VirtAddr) -> AxResult<()> {
        self.memory_set
            .lock().await
            .manual_alloc_range_for_lazy(start, end).await
    }

    /// alloc physical memory with the given type size for lazy allocation manually
    pub async fn manual_alloc_type_for_lazy<T: Sized>(&self, obj: *const T) -> AxResult<()> {
        self.memory_set
            .lock().await
            .manual_alloc_type_for_lazy(obj).await
    }

    /// 为进程分配一个文件描述符
    pub fn alloc_fd(&self, fd_table: &mut Vec<Option<Arc<dyn FileIO>>>) -> AxResult<usize> {
        for (i, fd) in fd_table.iter().enumerate() {
            if fd.is_none() {
                return Ok(i);
            }
        }
        if fd_table.len() >= self.fd_manager.get_limit() as usize {
            debug!("fd table is full");
            return Err(AxError::StorageFull);
        }
        fd_table.push(None);
        Ok(fd_table.len() - 1)
    }

}
