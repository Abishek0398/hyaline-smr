use crate::headnode::{HeadNode, NonAtomicHeadNode};
use crate::node::Node;
use crate::guard::Guard;
use crate::batch::BatchHandle;
use std::ptr::NonNull;
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
        0
    }

    unsafe fn traverse(traverse_node:NonNull<Node>, local_guard:&Guard) {
        let mut current = traverse_node.as_ref().list;
        loop {
            match current {
                Some(_) => {},
                None => {
                    break;
                },
            };
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
            if local_guard.is_handle(current) == true {
                break;
            }
            current = current.unwrap().as_ref().list;
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

    pub(crate) fn process_batch_handle(&self, batch_handle:&BatchHandle) {
        let mut batch_iter = batch_handle.iter();
        let nref_node = batch_handle.get_nref();
        let mut empty_slots:usize= 0;
        for slot in self.slots.iter() {
            if let Some(val) = batch_iter.next() {
                let add_result = slot.add_to_slot(val);
                match add_result {
                    Ok(val) => {
                        unsafe {
                            self.add_adjs(val.head_ptr,
                            val.head_count + self.adjs
                            );
                        }
                    },
                    Err(_) => empty_slots = empty_slots + 1,
                }
            }
        }
        if empty_slots > 0 {
            unsafe {self.add_adjs(nref_node,empty_slots.wrapping_mul(self.adjs));};
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
                  if let Some(act_traverse_node) = traverse_node {
                    unsafe {Collector::traverse(act_traverse_node,local_guard)};
                  }
                  break;
              },
              Err(_) => {},
           }
       }        
   }

   fn retire<T:'static>(&self,garbage : Box<T>) {
       let garb_node = Node::new(garbage);
       let res = BatchHandle::add_to_batch(self,garb_node);
       if let Err(_batch_handle) = res {
       }
   }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use super::Collector;
    use crate::collector::Smr;

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
        /*let loc_coll_1 = Arc::new(Collector::new());
        let loc_coll_2 = loc_coll_1.clone();
        let loc_coll_3 = loc_coll_1.clone();*/

        let first_garb = Box::new(TestNode{foo:1,bar:2});
        let second_garb = Box::new(TestNode{foo:2,bar:2});
        let third_garb = Box::new(TestNode{foo:3,bar:2});
        let fourth_garb = Box::new(TestNode{foo:4,bar:2});
        let five_garb = Box::new(TestNode{foo:5,bar:2});
        let six_garb = Box::new(TestNode{foo:6,bar:2});

        {
            let _guard1 = crate::pin();
            crate::retire(first_garb); 
            crate::retire(second_garb);
        }

        let handle1 = thread::spawn(move|| {
            {
                let _guard2 = crate::pin();
                crate::retire(third_garb); 
                crate::retire(fourth_garb);
            }
        });

        let handle2 = thread::spawn(move|| {
            {
                let _guard3 = crate::pin();
                crate::retire(five_garb); 
                crate::retire(six_garb);
            }
        });
        
        handle1.join().unwrap();
        handle2.join().unwrap();
    }
}