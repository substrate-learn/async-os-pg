// Could use `async_std::collections::HashMap` outsides. Great!
pub use hashbrown::{HashMap, HashSet};
// Could use `async_std::collections::Vec` outsides. Great!
#[macro_use]
pub mod vec;
pub use vec::Vec;
// Could use `async_std::collections::BinaryHeap` outsides. Great!
mod binary_heap;
pub use binary_heap::BinaryHeap;
