use std::cell::RefCell;
use std::sync::atomic::AtomicUsize;
use std::{marker::PhantomData, ptr::NonNull};

use atomicdouble::Ordering;

use crate::collector::Collector;
use crate::node::Node;

thread_local! {
    static LOCAL_BATCH:RefCell<BatchHandle> = RefCell::new(BatchHandle::new());
}

const BATCH_SIZE: usize = 64;

unsafe impl Send for BatchHandle {}

#[derive(Debug)]
pub struct BatchHandle {
    batch: *mut Batch,
    collector: *const Collector,
}

impl BatchHandle {
    fn new() -> Self {
        let res = Box::new(Batch::default());
        BatchHandle {
            batch: Box::into_raw(res),
            collector: std::ptr::null(),
        }
    }
    pub fn add_to_batch(collector: &Collector, val: Node) {
        LOCAL_BATCH.with(|b| {
            let mut handle = b.borrow_mut();
            handle.set_collector(collector);
            //This is safe because the batch pointer is always initialized when accesing the thread
            //local see new(). Also no other thread can access the batch as its local to each thread
            let res = unsafe { (*handle.batch).add(val) };
            if let Err(res_val) = res {
                let _filled_handle = BatchHandle {
                    batch: handle.batch,
                    collector: handle.collector,
                };

                handle.batch = Box::into_raw(Box::new(Batch::default()));

                unsafe {
                    let _ = (*handle.batch).add(res_val).unwrap();
                };
            }
        })
    }

    fn is_full() -> bool {
        LOCAL_BATCH.with(|b| -> bool { unsafe { (*b.borrow().batch).is_full() } })
    }

    fn get_size() -> usize {
        LOCAL_BATCH.with(|b| -> usize { unsafe { (*b.borrow().batch).get_size() } })
    }

    fn current_iter() -> Iter<'static> {
        LOCAL_BATCH.with(|b| -> Iter<'_> { unsafe { (*b.borrow().batch).iter() } })
    }

    pub fn get_node_nref(&self) -> Option<NonNull<Node>> {
        let handle = unsafe { &*self.batch };

        handle
            .first_node
            .as_ref()
            .map(|input| NonNull::from(input.as_ref()))
    }

    pub fn iter(&self) -> Iter<'_> {
        unsafe { (*self.batch).iter() }
    }

    fn set_collector(&mut self, collector: &Collector) {
        if self.collector.is_null() {
            self.collector = collector;
        }
    }
}

impl Drop for BatchHandle {
    fn drop(&mut self) {
        // This is safe because we null check the pointer and the pointer will always
        //point to the batch's active collector and the collector is of static scope or
        // it outlives the rest of the program.
        unsafe {
            if let Some(coll) = self.collector.as_ref() {
                coll.process_batch_handle(self);
            }
        }
    }
}
pub struct Batch {
    first_node: Option<Box<Node>>,
    size: usize,
    nref: AtomicUsize,
}

impl Batch {
    fn new() -> Self {
        Batch {
            first_node: None,
            size: 0,
            nref: AtomicUsize::new(0),
        }
    }

    fn iter(&mut self) -> Iter<'_> {
        let mut len: usize = 0;
        let current_node = self.first_node.as_ref().map(|input| {
            len = BATCH_SIZE;
            NonNull::from(input.as_ref())
        });
        Iter {
            current_node,
            len,
            marker: PhantomData,
        }
    }

    fn add(&mut self, mut val: Node) -> Result<(), Node> {
        if !self.is_full() {
            val.set_nref_node(NonNull::new(self));
            val.set_batch(self.first_node.take());
            self.first_node = Some(Box::new(val));
            self.size += 1;
            Ok(())
        } else {
            Err(val)
        }
    }

    fn is_full(&self) -> bool {
        if self.size == BATCH_SIZE {
            return true;
        }
        false
    }

    fn get_size(&self) -> usize {
        self.size
    }

    pub(crate) fn fetch_add_nref(&self, val: usize, ordering: Ordering) -> usize {
        self.nref.fetch_add(val, ordering)
    }

    pub(crate) fn fetch_sub_nref(&self, val: usize, ordering: Ordering) -> usize {
        self.nref.fetch_sub(val, ordering)
    }
}

impl Default for Batch {
    fn default() -> Self {
        Batch {
            first_node: None,
            size: 0,
            nref: AtomicUsize::new(0),
        }
    }
}

pub struct Iter<'a> {
    current_node: Option<NonNull<Node>>,
    len: usize,
    marker: PhantomData<&'a Node>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = NonNull<Node>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.len != 0 {
            let res = self.current_node;
            //safe because we check for the size of the batch above
            let mut_res: &mut Node = unsafe { res.unwrap().as_mut() };

            if self.len == 1 {
                self.current_node = None;
            } else {
                self.current_node = mut_res.produce_nodes_filler();
            }
            self.len -= 1;
            res
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::{collector::Collector, node::Node};

    use super::{Batch, BatchHandle, BATCH_SIZE};

    use lazy_static::lazy_static;

    lazy_static! {
        static ref COLLECTOR: Collector = Collector::new();
    }

    fn node_producer(i: usize) -> Node {
        if i % 2 == 0 {
            Node::new(Box::new(i))
        } else {
            Node::new(Box::new("er"))
        }
    }

    #[test]
    fn basic_batch_test() {
        for i in 1..2 * BATCH_SIZE {
            BatchHandle::add_to_batch(&COLLECTOR, node_producer(i));
            if i % BATCH_SIZE != 0 {
                assert!(!BatchHandle::is_full());
                assert!(BatchHandle::get_size() == i % BATCH_SIZE);
            } else {
                assert!(BatchHandle::is_full());
                assert!(BatchHandle::get_size() == BATCH_SIZE)
            }
        }
    }

    #[test]
    fn full_iterator_test() {
        let mut batch = Batch::default();
        for i in 1..BATCH_SIZE + 3 {
            let res = batch.add(node_producer(i));
            if i == BATCH_SIZE + 1 {
                assert!(res.is_err());
                break;
            }
        }
        let batch_iter = batch.iter();
        let mut count = 0;
        for node in batch_iter {
            count += 1;
            if count != BATCH_SIZE {
                unsafe { assert!(node.as_ref().is_present_batch_ptr()) }
            } else {
                unsafe { assert!(!node.as_ref().is_present_batch_ptr()) }
            }
        }
        assert_eq!(count, BATCH_SIZE);
    }

    #[test]
    fn partial_iterator_test() {
        let mut batch = Batch::default();
        for i in 1..BATCH_SIZE / 2 {
            let res = batch.add(node_producer(i));
            assert!(res.is_ok());
        }
        let batch_iter = batch.iter();
        let mut count = 0;
        for node in batch_iter {
            count += 1;
            if count != BATCH_SIZE {
                unsafe { assert!(node.as_ref().is_present_batch_ptr()) }
            } else {
                unsafe { assert!(!node.as_ref().is_present_batch_ptr()) }
            }
        }
        assert_eq!(count, BATCH_SIZE);
    }
}
