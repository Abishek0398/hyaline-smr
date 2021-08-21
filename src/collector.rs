use crate::headnode::HeadNode;
use crate::node::*;
use crate::guard::Guard;
use crate::batch::Batch;
use std::thread::{thread,ThreadId};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub struct Collector<const N:usize> {
    slots : [AtomicDouble::<HeadNode>;N]
}

impl<const N:usize> Collector<N> where
[AtomicDouble::<HeadNode>;N]:Default 
{
    fn new() -> Self {
        Collector{slots:Default::default()}
    }

    fn get_slot() -> usize {
        let mut hasher = DefaultHasher::new();
        threadid.hash(&mut hasher);
        hasher.finish()%N
    }
    fn add_adjs(node:Option<NonNull<Node>>) {
        let prev_val = node.unwrap_or_default().as_ref()
        .NrefNode.unwrap_or_default()
        .as_ref().nref
        .fetch_add(ADJS);
        if prev_val + ADJS == 0 {
            let x:Box<Batch> = Box::from_raw(node.unwrap_or_default().as_ref().NrefNode.un_wrap().as_mut_ptr());
        }
    }
    fn traverse() {
        
    }
}

pub trait smr {
    fn pin(&self) -> Guard;
    fn unpin(local_guard:&Guard);
    fn retire<T>(&self,garbage : Box<T>);
}
impl smr for Collector {
   fn pin(&self) -> Guard<'_> {
       let result_guard = Guard::new(self);
       result_guard.slot = Collector::get_slot();
       result_guard.handle = self
       .slots[result_guard.slot]
       .fetch_add(HeadNode::new(None,1))
       .head_ptr;
       result_guard
   }

   fn unpin(&self,local_guard:&Guard) {
       let start = local_guard.slot;
       let curr_head = HeadNode::Default();
       let traverse_node:Option<NonNull<Node>> = None;
       loop {
           curr_head = self.slots[start].load();
           cas_node = HeadNode::new(curr_head.head_ptr,curr_head.head_count.wrapping_sub(1));
           if curr_head.head_ptr != local_guard.handle {
                if let Some(ptr) = curr_head.head_ptr {
                    unsafe{traverse_node = ptr.as_ref().list;}
                }
           }
           if self.slots[start].compare_exchange(x,cas_node).is_ok() == true {
               break;
           }
       }
       if curr_head.head_count == 1 && curr_head.head_ptr !=None {
           Collector::add_adjs(curr_head.head_ptr);
       }
       Collector::traverse(traverse_node,local_guard.handle);   
   }

   fn retire<T>(&self,garbage : Box<T>) {
       let garb_node = Node::new(NodeVal::from(garbage));
       let res = BatchHandle::add_to_batch(garb_node);
       if res.is_err()==true {
            let batch_iter = res.expect_err().iter();
            for slot in slots {
                if let Some(val) = batch_iter.next() {
                    slot.insert(val);
                }
            }
       }
   }
}