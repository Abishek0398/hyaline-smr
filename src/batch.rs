use std::{cell::RefCell, marker::PhantomData, ptr::NonNull, sync::atomic::AtomicUsize};
use crate::node::Node;

thread_local!{
    static LOCAL_BATCH:RefCell<BatchHandle> = RefCell::new(BatchHandle::new());
}

const BATCH_SIZE:usize = 16;

unsafe impl Send for BatchHandle{}

#[derive(Debug)]
struct BatchHandle {
    batch:*mut Batch
}

impl BatchHandle {
    fn new()->Self {
        let res = Box::new(Batch::default());
        BatchHandle{batch:Box::into_raw(res)}
    }
    fn add_to_batch(val:Node) -> Result<(),BatchHandle> {
        LOCAL_BATCH.with(|b|->Result<(),BatchHandle> {
            let res = unsafe {
                (&mut*(b.borrow().batch)).add(val)
            };
            if res.is_err() == true {
                let ret_val = BatchHandle{batch:b.borrow().batch};
                b.borrow_mut().batch = Box::into_raw(Box::new(Batch::default()));
                unsafe {
                    let _ = (&mut*(b.borrow().batch)).add(res.err().unwrap_or_default())
                    .unwrap_or_default();
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
                (&*(b.borrow().batch)).is_full()
            }
        })
    }

    fn current_iter()->Iter<'static> {
        LOCAL_BATCH.with(|b|->Iter<'_> {
            unsafe  {
                (&mut*(b.borrow().batch)).iter()
            }
        })
    }

    fn iter(&self)->Iter<'_> {
        unsafe  {
            (&mut*(self.batch)).iter()
        }
    }
}

pub struct Batch {
    first_node: Option< Box<Node> >,
    size: usize,
    nref: AtomicUsize
}

impl Batch {
    fn new()->Self {
        Batch{first_node:None,size:0,nref:AtomicUsize::default()}
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
            nref:AtomicUsize::default()
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

#[cfg(test)]
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
}