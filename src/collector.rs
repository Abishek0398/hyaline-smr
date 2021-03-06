use std::ptr::NonNull;

use crate::batch::BatchHandle;
use crate::guard::Guard;
use crate::headnode::HeadNode;
use crate::node::Node;

use crate::primitive::thread;

const SLOTS_LENGTH: usize = 64;

pub(crate) const ADJS: usize = (usize::MAX / SLOTS_LENGTH).wrapping_add(1);

/// Garbage collector that implements Hyaline algorithm
/// Number of slots is fixed at present with 32 slots
#[derive(Debug)]
pub struct Collector {
    slots: [HeadNode; SLOTS_LENGTH],
}

impl Collector {
    /// Creates a new collector with default configurations.
    ///
    /// It is absolutely essential for the collector obtained here to live longer than
    /// all the threads that use it. Preferrably use this function to initialize a collector in static scope
    /// or if possible use scoped threads.
    #[rustfmt::skip]
    pub const fn new() -> Self {
        Collector {
            slots: [
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                HeadNode::new(None, 0),
                ],
        }
    }

    fn get_slot() -> usize {
        let thread_id: usize = thread::current().id().as_u64().get() as usize;
        thread_id % SLOTS_LENGTH
    }

    pub(crate) fn process_batch_handle(&self, batch_handle: &mut BatchHandle) {
        let mut batch_iter = batch_handle.iter();
        let mut empty_slots: usize = 0;
        let node_nref = batch_handle.get_node_nref();
        for slot in self.slots.iter() {
            if let Some(mut val) = batch_iter.next() {
                let add_result = unsafe { slot.add_to_slot(val.as_mut()) };
                match add_result {
                    Ok(_) => {}
                    Err(_) => empty_slots += 1,
                }
            }
        }
        if empty_slots > 0 {
            unsafe {
                Node::add_to_nref(node_nref, empty_slots.wrapping_mul(ADJS));
            };
        }
    }
}

impl Default for Collector {
    fn default() -> Self {
        Collector::new()
    }
}

/// Safe Memory Reclaimation(SMR) trait defines the methods that a collector must implement
/// and expose to the end user. The methods defined here are standard APIs in the concurrent
/// garbage collector world. These APIs are also used in other SMR schemes like epoch based garbage collector
pub trait Smr {
    /// Registers a thread to the garbage collector. Any operation to a
    /// concurrent data structure, has to be performed with a guard in scope.(i.e, Before any operation call the hyaline::pin() method)
    /// This method returna a guard, which takes care of unpining the thread when the guard goes out of scope.
    /// Exact details on what constitutes a register is implementation dependent
    fn pin(&self) -> Guard<'_>;

    /// This is the opposite of pin method. Upon calling this method the thread will be de-registered.
    /// Most implementations dont expose this method to the end user as it it will be put behind a RAII guard.
    fn unpin(&self, local_guard: &Guard<'_>);

    /// Collects the garbage values form the user. The local_guard argument is just here
    /// for ensuring that retire() is called after a pin().
    ///
    /// # Safety
    /// Caller must ensure that only logically deleted values of the concerned data structure is
    /// provided to the retire method. For example: In a lock-free linkedlist retire() needs to be called
    /// only after the concerned node is removed form the list.
    unsafe fn retire<T>(&self, garbage: Option<NonNull<T>>, _local_guard: &Guard<'_>);
}

impl Smr for Collector {
    fn pin(&self) -> Guard<'_> {
        let mut result_guard = Guard::new(self);
        result_guard.slot = Collector::get_slot();
        result_guard.handle = self.slots[result_guard.slot].pin_slot();
        result_guard
    }

    fn unpin(&self, local_guard: &Guard<'_>) {
        let start = local_guard.slot;
        self.slots[start].unpin_slot(local_guard);
    }

    unsafe fn retire<T>(&self, garbage: Option<NonNull<T>>, _local_guard: &Guard<'_>) {
        if let Some(garb) = garbage {
            let garb_node = Node::new(Box::from_raw(garb.as_ptr()));
            BatchHandle::add_to_batch(self, garb_node);
        }
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use std::{
        ptr::NonNull,
        sync::atomic::{AtomicUsize, Ordering},
        thread,
    };

    use crate::{Collector, Smr};

    const MAX_THREADS: usize = 8;
    static COLLECTOR: Collector = Collector::new();
    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

    struct TestNode {
        foo: usize,
        bar: usize,
    }

    impl Drop for TestNode {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn node_producer(i: usize) -> Option<NonNull<TestNode>> {
        if i % 2 == 0 {
            let x = Box::new(TestNode { foo: i, bar: i + 1 });
            NonNull::new(Box::into_raw(x))
        } else {
            let y = Box::new(TestNode { foo: 0, bar: 0 });
            NonNull::new(Box::into_raw(y))
        }
    }
    #[test]
    fn count_drop() {
        let mut handle_array = Vec::new();

        for _i in 0..MAX_THREADS {
            let handle = thread::spawn(move || {
                let guard = COLLECTOR.pin();
                for j in 0..5000 {
                    unsafe {
                        COLLECTOR.retire(node_producer(j), &guard);
                    }
                }
            });
            handle_array.push(handle);
        }
        while DROP_COUNT.load(Ordering::Relaxed) < MAX_THREADS * 5000 {}
        assert_eq!(DROP_COUNT.load(Ordering::Relaxed), MAX_THREADS * 5000);
    }
}
