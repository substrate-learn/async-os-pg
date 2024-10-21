pub use taskctx::TaskStack;
use alloc::vec::Vec;

/// A simple stack pool
pub(crate) struct StackPool {
    free_stacks: Vec<TaskStack>,
    current: Option<TaskStack>,
}

impl StackPool {
    /// Creates a new empty stack pool.
    pub const fn new() -> Self {
        Self {
            free_stacks: Vec::new(),
            current: None,
        }
    }

    pub fn init(&mut self) {
        self.current = Some(TaskStack::new_init());
    }

    /// Alloc a free stack from the pool.
    fn alloc(&mut self) -> TaskStack {
        self.free_stacks.pop().unwrap_or_else(|| {
            let stack = TaskStack::alloc(axconfig::TASK_STACK_SIZE);
            stack
        })
    }

    // /// Recycle a stack to the pool.
    // pub fn recycle(&mut self, stack: TaskStack) {
    //     self.free_stacks.push(stack);
    // }

    pub fn pick_current_stack(&mut self) -> TaskStack{
        assert!(self.current.is_some());
        let new_stack = self.alloc();
        self.current.replace(new_stack).unwrap()
    }

    pub fn current_stack(&self) -> &TaskStack {
        assert!(self.current.is_some());
        self.current.as_ref().unwrap()
    }

    pub fn put_prev_stack(&mut self, kstack: TaskStack) {
        assert!(self.current.is_some());
        let curr_stack = self.current.replace(kstack).unwrap();
        self.free_stacks.push(curr_stack);
    }

}