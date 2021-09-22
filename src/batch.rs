use std::cell::UnsafeCell;
use std::sync::atomic::AtomicUsize;
use std::{marker::PhantomData, ptr::NonNull};

use crate::collector::Collector;
use crate::node::Node;

thread_local!{
    static LOCAL_BATCH:UnsafeCell<BatchHandle> = UnsafeCell::new(BatchHandle::new());
}

const BATCH_SIZE:usize = 32;

unsafe impl Send for BatchHandle{}

#[derive(Debug)]
pub struct BatchHandle {
    batch:*mut Batch,
    collector: Option<*const Collector>,
}

impl BatchHandle {
    fn new()->Self {
        let res = Box::new(Batch::default());
        BatchHandle{
            batch:Box::into_raw(res),
            collector: None
        }
    }
    pub fn add_to_batch(collector:&Collector, val:Node) -> Result<(),BatchHandle> {
        LOCAL_BATCH.with(|b|->Result<(),BatchHandle> {
            let handle = unsafe {
                b.get().as_mut().unwrap()
            };
            let res = unsafe {
                handle.set_collector(collector);
                (*handle.batch).add(val)
            };
            if res.is_err() == true {
                let ret_val = BatchHandle{
                    batch:handle.batch,
                    collector:handle.collector
                };

                handle.batch = Box::into_raw(Box::new(Batch::default()));

                unsafe {
                    let _ = (*handle.batch).add(res.err().unwrap())
                    .unwrap();
                };
                Err(ret_val)
            }
            else {
                Ok(())
            }
        })
    }

    fn is_full()->bool {
        LOCAL_BATCH.with(|b|->bool {
            unsafe  {
                (*b.get().as_ref().unwrap().batch).is_full()
            }
        })
    }

    fn current_iter()->Iter<'static> {
        LOCAL_BATCH.with(|b|->Iter<'_> {
            unsafe  {
                (*b.get().as_ref().unwrap()).iter()
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
        if let None = self.collector {
            self.collector = Some(collector);
        }
    }
}

impl Drop for BatchHandle {
    fn drop(&mut self) {
        unsafe {
            if let Some(val)=self.collector {
                val.as_ref().unwrap().process_batch_handle(self);
            }
            else {
                println!("ffs");
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
            val.nref_node = NonNull::new(self as *mut Batch);
            val.batch = self.first_node.take();
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
            let mut res = self.current_node;
            let mut_res:&mut Node = unsafe {

                res.as_mut().unwrap().as_mut()
            };
            self.current_node = match mut_res.batch.as_mut() {
                Some(val) => {
                    NonNull::new(val.as_mut() as *mut Node)
                },

                None => {
                    if self.len ==1 {
                        None 
                    }
                    else {
                        let mut batch_filler = Box::new(Node::default());
                        batch_filler.nref_node = mut_res.nref_node;
                        let return_val = NonNull::new(batch_filler.as_mut() as *mut Node);
                        mut_res.batch = Some(batch_filler);
                        return_val
                    }
                }
            };
            self.len = self.len-1;
            res
        }
        else {
            None
        }
    }
}

/*#[cfg(test)]
mod tests {

    use std::convert::TryInto;

    use crate::node::{Node};

    use super::{BATCH_SIZE, BatchHandle};

    fn node_producer(i:u32)->Node {
        if i%2 == 0 {
            Node::new(Box::new(i))
        }
        else {
            Node::new(Box::new("er"))
        }
        
    }

    #[test]
    fn iterator_test() {
        for i in 1..40 {
            let handle = 
            BatchHandle::add_to_batch(node_producer(i));
            let _ = match handle{
                Ok(_) => {
                    let max_size:u32 = BATCH_SIZE.try_into().unwrap();
                    if i%max_size != 0 {
                        assert!(!BatchHandle::is_full());
                    }
                    else {
                        assert!(BatchHandle::is_full());
                    }
                },
                Err(handle) => {
                    unsafe {
                        for garb in handle.iter() {
                                let act_node = garb.as_ref();
                                assert!(act_node.nref_node.unwrap().as_ref().is_full());
                            }
                        let _ = Box::from_raw(handle.batch);
                    }
                },
            };
        }
    }
}*/