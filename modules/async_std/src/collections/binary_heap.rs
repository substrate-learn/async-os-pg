use super::vec::Vec;

pub struct BinaryHeap<T> {
    heap: Vec<T>,
    len: usize,
}

impl<T> BinaryHeap<T> {
    pub fn new() -> Self {
        Self {
            heap: Vec::new(),
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn peek(&self) -> Option<&T> {
        self.heap.get(0)
    }

    fn need_inflation(&self) -> bool {
        self.len == self.heap.len()
    }

    fn need_truncate(&self) -> bool {
        self.len <= (self.heap.len() / 4)
    }
}

impl<T: Ord> BinaryHeap<T> {
    pub fn push(&mut self, value: T) {
        if self.need_inflation() {
            self.heap.push(value);
        } else {
            self.heap[self.len] = value;
        }

        let mut i = self.len;
        while i > 0 {
            let parent = (i - 1) / 2;
            if self.heap[i] > self.heap[parent] {
                self.heap.swap(i, parent);
            }

            i = parent;
        }

        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        // Move the value out of the vector. It's not so simple!
        let result = unsafe {
            self.len -= 1;
            Some(core::ptr::read(self.heap.as_ptr()))
        };

        unsafe {
            self.heap[0] = core::ptr::read(self.heap.as_ptr().add(self.len()));
        }

        let mut i = 0usize;
        while i < self.len {
            let left = 2 * i + 1;
            let right = 2 * i + 2;
            let j = if right < self.len {
                if self.heap[left] > self.heap[right] {
                    left
                } else {
                    right
                }
            } else if left < self.len {
                left
            } else {
                break;
            };

            if self.heap[i] < self.heap[j] {
                self.heap.swap(i, j);
                i = j;
            } else {
                break;
            }
        }

        if self.need_truncate() {
            self.heap.truncate(self.heap.len() / 4 + 1);
        }

        result
    }
}
