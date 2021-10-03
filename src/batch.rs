use std::cell::RefCell;
use std::sync::atomic::AtomicUsize;
use std::{marker::PhantomData, ptr::NonNull};

use crate::collector::Collector;
use crate::node::Node;

thread_local!{
    static LOCAL_BATCH:RefCell<BatchHandle> = RefCell::new(BatchHandle::new());
}

const BATCH_SIZE:usize = 32;

unsafe impl Send for BatchHandle{}

#[derive(Debug)]
pub struct BatchHandle {
    batch: *mut Batch,
    collector: *const Collector
}

impl BatchHandle {
    fn new()->Self {
        let res = Box::new(Batch::default());
        BatchHandle{
            batch:Box::into_raw(res),
            collector: std::ptr::null(),
        }
    }
    pub fn add_to_batch(collector:&Collector, val:Node) {
        LOCAL_BATCH.with(|b| {
            let mut handle = b.borrow_mut();
            handle.set_collector(collector);
            let res = unsafe {
                (*handle.batch).add(val)
            };
            if let Err(res_val) = res {
                let _filled_handle = BatchHandle{
                    batch:handle.batch,
                    collector:handle.collector,
                };

                handle.batch = Box::into_raw(Box::new(Batch::default()));

                unsafe {
                    let _ = (*handle.batch).add(res_val)
                    .unwrap();
                };
            }
        })
    }

    fn is_full()->bool {
        LOCAL_BATCH.with(|b|->bool {
            unsafe  {
                (*b.borrow().batch).is_full()
            }
        })
    }

    fn get_size()->usize {
        LOCAL_BATCH.with(|b|->usize {
            unsafe  {
                (*b.borrow().batch).get_size()
            }
        })
    }

    fn current_iter()->Iter<'static> {
        LOCAL_BATCH.with(|b|->Iter<'_> {
            unsafe  {
                (*b.borrow().batch).iter()
            }
        })
    }

    pub fn get_nref(&self)->Option<NonNull<Node>> {
        unsafe {
            (*self.batch).first_node.as_mut()
            .and_then(|input|->Option< NonNull<Node> > {
                NonNull::new(input.as_mut() as *mut Node)
            })
        }
    }
    pub fn iter(&self)->Iter<'_> {
        unsafe  {
            (*self.batch).iter()
        }
    }
    fn set_collector(&mut self, collector:&Collector) {
        if self.collector == std::ptr::null(){
            self.collector = collector;
        }
    }
}

impl Drop for BatchHandle {
    fn drop(&mut self) {
        unsafe {
            if let Some(coll) = self.collector.as_ref() {
                coll.process_batch_handle(self);
            }
        }
    }
}
pub struct Batch {
    first_node: Option< Box<Node> >,
    size: usize,
    pub nref: AtomicUsize
}

impl Batch {
    fn new()->Self {
        Batch{first_node:None,size:0,nref:AtomicUsize::new(0)}
    }

    fn iter(&mut self)->Iter<'_> {
        let mut len:usize = 0;
        let current_node = self.first_node.as_mut().and_then(|input|->Option< NonNull<Node> > {
            len=BATCH_SIZE;
            NonNull::new(input.as_mut() as *mut Node)
        });
        Iter{current_node:current_node,len:len,marker:PhantomData}
    }

    fn add(&mut self,mut val:Node) -> Result<(),Node> {
        if self.is_full() == false {
            val.set_nref_node(NonNull::new(self as *mut Batch));
            val.set_batch(self.first_node.take());
            self.first_node = Some(Box::new(val));
            self.size = self.size + 1;
            Ok(())
        }
        else {
           Err(val) 
        }
    }

    fn is_full(&self) -> bool {
        if self.size == BATCH_SIZE {
            return true
        }
        false
    }

    fn get_size(&self) -> usize {
       self.size
    }
}

impl Default for Batch {
    fn default() -> Self {
        Batch{
            first_node:None,
            size:0,
            nref:AtomicUsize::new(0)
        }
    }
}

pub struct Iter<'a> {
    current_node: Option<NonNull<Node>>,
    len: usize,
    marker: PhantomData<&'a Node>
}

impl<'a> Iterator for Iter<'a> {
    type Item = NonNull<Node>;
    fn next(&mut self)->Option<Self::Item> {
        if self.len !=0 {
            let res = self.current_node;
            let mut_res:&mut Node = unsafe {
                res.unwrap().as_mut()
            };
            
            if self.len == 1 {
                self.current_node = None;
            }
            else {
                self.current_node = mut_res.produce_nodes_filler();
            }
            self.len = self.len-1;
            res
        }
        else {
            None
        }
    }
}

#[cfg(test)]
mod tests {

    use std::convert::TryInto;

    use crate::{collector::Collector, node::{Node}};

    use super::{BATCH_SIZE, BatchHandle};

    use lazy_static::lazy_static;

    lazy_static! {
        static ref COLLECTOR: Collector = Collector::new();
    }

    fn node_producer(i:u32)->Node {
        if i%2 == 0 {
            Node::new(Box::new(i))
        }
        else {
            Node::new(Box::new("er"))
        }    
    }

    #[test]
    fn basic_batch_test() {
        for i in 1..40 {
            BatchHandle::add_to_batch(&COLLECTOR,node_producer(i));
            let act_i:usize = i.try_into().unwrap();
            if act_i%BATCH_SIZE != 0 {
                assert!(!BatchHandle::is_full());
                assert!(BatchHandle::get_size() == act_i%BATCH_SIZE);
            }
            else {
                assert!(BatchHandle::is_full());
                assert!(BatchHandle::get_size() == BATCH_SIZE)
            }
        }
    }
}