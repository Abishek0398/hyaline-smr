/// Garbage collector that implements Hyaline algorithm
///
/// # Examples
///
/// ```
/// use hyaline::Collector;
///
/// let collector = Collector::new();
///
/// collector.pin();
///
/// // This line is just an example.
/// let del_node:Option<NonNull<Node>> = lock_free_ds.get_some();
///
/// unsafe {
///     collector.retire(del_node);
/// }
/// /*
/// perform other operations on the lock free data structure
///  */
///
/// ```
use rand::{thread_rng, Rng};

use crate::batch::BatchHandle;
use crate::guard::Guard;
use crate::headnode::{HeadNode, NonAtomicHeadNode};
use crate::node::Node;

use std::ptr::NonNull;
use std::sync::atomic::Ordering;

const SLOTS_LENGTH: usize = 32;

/// Garbage collector that implements Hyaline algorithm
///
/// Number of slots is fixed at present with 32 slots
#[derive(Debug)]
pub struct Collector {
    slots: [HeadNode; SLOTS_LENGTH],
    adjs: usize,
}

impl Collector {
    /// Creates a new collector with default configurations.
    ///
    /// It is absolutely essential for the collector obtained here to live longer than
    /// all the threads that use it. Preferrably use this function to initialize a collector in static scope
    /// or if possible use scoped threads.
    pub fn new() -> Self {
        Collector {
            slots: Default::default(),
            adjs: (usize::MAX / SLOTS_LENGTH).wrapping_add(1),
        }
    }

    fn get_slot() -> usize {
        thread_rng().gen_range(0..SLOTS_LENGTH)
    }

    pub(crate) fn process_batch_handle(&self, batch_handle: &mut BatchHandle) {
        let mut batch_iter = batch_handle.iter();
        let mut empty_slots: usize = 0;
        let nref_node = batch_handle.get_node_nref();
        for slot in self.slots.iter() {
            if let Some(mut val) = batch_iter.next() {
                let add_result = unsafe { slot.add_to_slot(val.as_mut()) };
                match add_result {
                    Ok(val) => unsafe {
                        Node::add_adjs(val.head_ptr, val.head_count + self.adjs);
                    },
                    Err(_) => empty_slots += 1,
                }
            }
        }
        if empty_slots > 0 {
            unsafe {
                Node::add_adjs(nref_node, empty_slots.wrapping_mul(self.adjs));
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
/// and expose to the end user. The methods defined here are standard APIs in the lockfree
/// garbage collector world. These APIs are also used in other SMR schemes like epoch based garbage collector
pub trait Smr {
    /// Registers a thread to the garbage collector. Any operation to a
    /// lock free data structure needs to be performed with a guard in scope.(i.e, Before any operation call the hyaline::pin() method)
    /// This method returna a guard, which takes care of unpining the thread when the guard goes out of scope.
    /// Exact details on what constitutes a register is implementation dependent
    fn pin(&self) -> Guard;

    /// This is the opposite of pin method. Upon calling this method the thread will be de-registered.
    /// Most implementations dont expose this method to the end user as it it will be put behind a RAII guard.
    fn unpin(&self, local_guard: &Guard);

    /// Collects the garbage values form the user. The local_guard argument is just here
    /// for ensuring that retire() is called after a pin().
    ///
    /// # Safety
    /// Caller must ensure that only logically deleted values of the concerned data structure is
    /// provided to the retire method. For example: In a lock-free linkedlist retire() needs to be called
    /// only after the concerned node is removed form the list.
    unsafe fn retire<T>(&self, garbage: Option<NonNull<T>>, _local_guard: &Guard);
}

impl Smr for Collector {
    fn pin(&self) -> Guard<'_> {
        let mut result_guard = Guard::new(self);
        result_guard.slot = Collector::get_slot();
        result_guard.handle = self.slots[result_guard.slot].pin_slot();
        result_guard
    }

    fn unpin(&self, local_guard: &Guard) {
        let start = local_guard.slot;
        loop {
            let curr_head: NonAtomicHeadNode = self.slots[start].load(Ordering::Relaxed);
            let unpin_result = self.slots[start].unpin_slot(curr_head, local_guard);
            if let Ok(traverse_node) = unpin_result {
                if curr_head.head_count == 1 && curr_head.head_ptr != None {
                    unsafe {
                        Node::add_adjs(curr_head.head_ptr, self.adjs);
                    };
                }
                if let Some(act_traverse_node) = traverse_node {
                    unsafe {
                        act_traverse_node.as_ref().traverse(local_guard);
                    };
                }
                break;
            }
        }
    }

    unsafe fn retire<T>(&self, garbage: Option<NonNull<T>>, _local_guard: &Guard) {
        if let Some(garb) = garbage {
            let garb_node = Node::new(Box::from_raw(garb.as_ptr()));
            BatchHandle::add_to_batch(self, garb_node);
        }
    }
}

#[cfg(test)]
mod tests {
    use lazy_static::lazy_static;
    use std::{
        ptr::NonNull,
        sync::atomic::{AtomicUsize, Ordering},
        thread,
    };

    use crate::collector::{Collector, Smr};

    const MAX_THREADS: usize = 8;
    lazy_static! {
        static ref COLLECTOR: Collector = Collector::new();
        static ref DROP_COUNT: AtomicUsize = AtomicUsize::new(0);
    }

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
