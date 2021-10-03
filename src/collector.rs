use rand::{Rng, thread_rng};

use crate::headnode::{HeadNode, NonAtomicHeadNode};
use crate::node::Node;
use crate::guard::Guard;
use crate::batch::BatchHandle;

use std::sync::atomic::Ordering;


const SLOTS_LENGTH:usize = 32;

#[derive(Debug)]
pub struct Collector {
    slots : [HeadNode;SLOTS_LENGTH],
    adjs : usize
}

impl Collector
{
    pub fn new() -> Self {
        Collector{
            slots:Default::default(),
            adjs:(usize::MAX/SLOTS_LENGTH).wrapping_add(1)
        }
    }

    fn get_slot() -> usize {
        thread_rng().gen_range(0..SLOTS_LENGTH)
    }

    pub(crate) fn process_batch_handle(&self, batch_handle:&BatchHandle) {
        let mut batch_iter = batch_handle.iter();
        let mut empty_slots:usize= 0;
        let nref_node = batch_handle.get_nref();
        for slot in self.slots.iter() {
            if let Some(val) = batch_iter.next() {
                let add_result = unsafe {slot.add_to_slot(val)};
                match add_result {
                    Ok(val) => {
                        unsafe {
                            Node::add_adjs(val.head_ptr,
                            val.head_count + self.adjs
                            );
                        }
                    },
                    Err(_) => empty_slots = empty_slots + 1,
                }
            }
        }
        if empty_slots > 0 {
            unsafe {Node::add_adjs(nref_node,empty_slots.wrapping_mul(self.adjs));};
        }
    }
}
pub trait Smr {
    fn pin(&self) -> Guard;
    fn unpin(&self,local_guard:&Guard);
    fn retire<T:'static>(&self,garbage : Box<T>);
}
impl Smr for Collector {
   fn pin(&self) -> Guard<'_> {
       let mut result_guard = Guard::new(self);
       result_guard.slot = Collector::get_slot();
       result_guard.handle = self
       .slots[result_guard.slot]
       .pin_slot();
       result_guard
   }

   fn unpin(&self,local_guard:&Guard) {
       let start = local_guard.slot;
       loop {
           let curr_head:NonAtomicHeadNode = self.slots[start].load(Ordering::SeqCst);
           let unpin_result = self.slots[start].unpin_slot(curr_head,local_guard);
           match unpin_result {
              Ok(traverse_node) => {
                  if curr_head.head_count == 1 && curr_head.head_ptr !=None {
                       unsafe {
                           Node::add_adjs(curr_head.head_ptr,self.adjs);
                        };
                  }
                  if let Some(act_traverse_node) = traverse_node {
                        unsafe {
                            act_traverse_node.as_ref().traverse(local_guard);
                        };
                  }
                  break;
              },
              Err(_) => {},
           }
       }        
   }

   fn retire<T:'static>(&self,garbage : Box<T>) {
       let garb_node = Node::new(garbage);
       BatchHandle::add_to_batch(self,garb_node);
   }
}

#[cfg(test)]
mod tests {
    use std::{sync::atomic::{AtomicUsize,Ordering}, thread};
    use lazy_static::lazy_static;

    use crate::collector::{Collector, Smr};

    const MAX_THREADS:usize = 8;
    lazy_static! {
        static ref COLLECTOR: Collector = Collector::new();
        static ref DROP_COUNT: AtomicUsize = AtomicUsize::new(0);
    }

    struct TestNode {
        foo:usize,
        bar:usize
    }

    impl Drop for TestNode {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn node_producer(i:usize)->Box<TestNode> {
        if i%2 == 0 {
            Box::new(TestNode{foo:i,bar:i+1})
        }
        else {
            Box::new(TestNode{foo:0,bar:0})
        }     
    }
    #[test]
    fn count_drop() {
        let mut handle_array = Vec::new();

        for _i in 0..MAX_THREADS {
            let handle = thread::spawn(move|| {
                let _guard = COLLECTOR.pin();
                for j in 0..5000 {   
                    COLLECTOR.retire(node_producer(j));                 
                }
            });
            handle_array.push(handle);
        }
        while DROP_COUNT.load(Ordering::Relaxed)<MAX_THREADS*5000 {}; 
        assert_eq!(DROP_COUNT.load(Ordering::Relaxed),MAX_THREADS*5000);
    }
}