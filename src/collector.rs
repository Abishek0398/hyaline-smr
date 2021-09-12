use crate::headnode::{HeadNode, NonAtomicHeadNode};
use crate::node::Node;
use crate::guard::Guard;
use crate::batch::BatchHandle;
use std::ptr::NonNull;
use crate::sync::Ordering;

const SLOTS_LENGTH:usize = 1;
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
        0
    }

    unsafe fn traverse(traverse_node:Option<NonNull<Node>>, local_guard:&Guard) {
        let mut current = traverse_node;
        while local_guard.is_handle(current) == false {
            let next = current.unwrap().as_ref().list;
            let prev_val = current.unwrap().as_ref()
            .nref_node.unwrap()
            .as_ref()
            .nref
            .fetch_sub(1,Ordering::SeqCst);
            if prev_val.wrapping_sub(1) == 0 {
                let _ = Box::from_raw(current.unwrap()
                .as_ref()
                .nref_node
                .unwrap()
                .as_ptr());
            }
            current = next;
        }
    }

    unsafe fn add_adjs(&self,node:Option<NonNull<Node>>, val:usize) {
        match node {
            Some(node_val) => {
                let prev_val = node_val.as_ref()
                .nref_node.unwrap()
                .as_ref()
                .nref
                .fetch_add(val,Ordering::SeqCst);
                if prev_val.wrapping_add(val) == 0 {
                    let _ = Box::from_raw(node.unwrap()
                    .as_ref()
                    .nref_node
                    .unwrap()
                    .as_ptr());
                }
            },
            None => {},
        };
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
       .fetch_add(None,1,Ordering::Relaxed)
       .get_guard_handle();
       result_guard
   }

   fn unpin(&self,local_guard:&Guard) {
       let start = local_guard.slot;
       loop {
           let curr_head:NonAtomicHeadNode = self.slots[start].load(Ordering::Relaxed);
           let unpin_result = self.slots[start].unpin_slot(curr_head,local_guard);
           match unpin_result {
              Ok(traverse_node) => {
                  if curr_head.head_count == 1 && curr_head.head_ptr !=None {
                       unsafe {
                           self.add_adjs(curr_head.head_ptr,self.adjs);
                        };
                  }
                  unsafe {Collector::traverse(traverse_node,local_guard);}
                  break;
              },
              Err(_) => {},
           }
       }        
   }

   fn retire<T:'static>(&self,garbage : Box<T>) {
       let garb_node = Node::new(garbage);
       let res = BatchHandle::add_to_batch(garb_node);
       if let Err(batch_handle) = res {
            let mut batch_iter = batch_handle.iter();
            let nref_node = batch_handle.get_nref();
            let mut empty_slots= 0;
            for slot in self.slots.iter() {
                if let Some(val) = batch_iter.next() {
                    let add_result = slot.add_to_slot(val);
                    match add_result {
                        Ok(val) => {
                            unsafe {
                                self.add_adjs(val.0,
                                val.1 + self.adjs
                                );
                            }
                        },
                        Err(_) => empty_slots = empty_slots + self.adjs,
                    }
                }
            }
            if empty_slots != 0 {
                unsafe {self.add_adjs(nref_node,empty_slots);};
            }
       }
   }
}

#[cfg(test)]
mod tests {
    use loom::thread;

    use crate::retire;
    use crate::pin;

    struct TestNode {
        foo:i32,
        bar:i32
    }
    impl Drop for TestNode {
        fn drop(&mut self) {
            println!("Testnode drop woo hoo : {}", self.foo);
        }
    }
    #[test]
    fn collector_test() {
        loom::model(|| {
            let first_garb = Box::new(TestNode{foo:1,bar:2});
            let second_garb = Box::new(TestNode{foo:2,bar:2});
            let third_garb = Box::new(TestNode{foo:3,bar:2});
            let fourth_garb = Box::new(TestNode{foo:4,bar:2});
            {
                let _guard = pin();
                retire(first_garb); 
                retire(second_garb);
            }

            let handle = thread::spawn(|| {
                {let _guard = pin();
                retire(third_garb); 
                retire(fourth_garb);}         
            });
            handle.join().unwrap();
        });
    }
}