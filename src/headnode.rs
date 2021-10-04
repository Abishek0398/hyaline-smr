use std::ptr::NonNull;

use atomicdouble::AtomicDouble;
use atomicdouble::Ordering;

use crate::guard::Guard;
use crate::node::Node;

#[derive(Debug)]
pub(crate)struct HeadNode {
    head : AtomicDouble<NonAtomicHeadNode>
}

impl HeadNode {
    pub(crate) fn new(head_ptr:Option< NonNull<Node> >, head_count:usize)->Self {
        HeadNode {
            head : AtomicDouble::new(
                NonAtomicHeadNode {
                    head_ptr:head_ptr,
                    head_count:head_count
                }
            )
        }
    }

    pub(crate) fn add_to_slot(&self, val:&mut Node)->
    Result<NonAtomicHeadNode,()> {
        let mut curr_node = self.head.load(Ordering::SeqCst);

        loop {
            if curr_node.head_count == 0 {
                val.set_list(None);
                return Err(());
            }
            val.set_list(curr_node.head_ptr);

            let new_node = NonAtomicHeadNode {
                head_ptr:NonNull::new(val),
                head_count:curr_node.head_count
            };

            let cxchg_result = self.head.compare_exchange(curr_node,
                new_node,
        Ordering::SeqCst,
        Ordering::SeqCst
            );
            match cxchg_result {
                Ok(_) => {
                    return Ok(curr_node);
                },
                Err(node) => curr_node = node,
            };
        }
    }

    pub(crate)fn pin_slot(&self)->Option<&'static Node> {
        unsafe {
            // Safe because the headptr obtained from NonAtomicHeadNode returned
            // from fetchadd is either valid(The algorithm ensures this) or None
            self.fetch_add(None,1,Ordering::SeqCst)
            .get_guard_handle()
        }
    }

    pub(crate) fn unpin_slot(&self,curr_head:NonAtomicHeadNode, local_guard:&Guard)->Result<Option<NonNull<Node>>,()> {
        let mut traverse_node = None;
        let cas_node = {
            if curr_head.head_count == 1 {
                NonAtomicHeadNode::new(None,0)
            }
            else {
                NonAtomicHeadNode::new(curr_head.head_ptr,curr_head.head_count.wrapping_sub(1))
            }
        };

        if local_guard.is_handle(curr_head.head_ptr) == false {
            traverse_node = curr_head.head_ptr.and_then(|val| {
                unsafe {
                    val.as_ref().get_list()
                }
            });
        }
        match self.head.compare_exchange(curr_head
            ,cas_node
            ,Ordering::SeqCst
            ,Ordering::SeqCst
        ) {
            Ok(_) => Ok(traverse_node),
            Err(_) => Err(()),
        }
    }

    pub(crate) fn load(&self, order:Ordering)->NonAtomicHeadNode {
        self.head.load(order)
    }

    pub(crate) fn fetch_add(&self,head_ptr:Option<NonNull<Node>>,head_cnt:usize,order:Ordering)->NonAtomicHeadNode {
        let add_node = NonAtomicHeadNode::new(head_ptr,head_cnt);
        self.head.fetch_add(add_node,order)
    }
}

impl Default for HeadNode {
    fn default() -> Self {
        Self { head: Default::default() }
    }
}

#[derive(Debug)]
pub(crate) struct NonAtomicHeadNode {
    pub head_ptr : Option< NonNull<Node> >,
    pub head_count : usize
}

/*
Grossly unsafe but our implementation guarantees that there is no data race
or UB of any kind when sending this type
*/
unsafe impl Send for NonAtomicHeadNode{}

impl Copy for NonAtomicHeadNode {}

impl Clone for NonAtomicHeadNode {
    fn clone(&self) -> Self {
        *self
    }
}

impl Default for NonAtomicHeadNode {
    fn default()->Self {
        NonAtomicHeadNode {head_ptr:None,head_count:0}
    }
}

impl NonAtomicHeadNode {
    pub fn new(ptr:Option< NonNull<Node> >,cnt: usize)->Self {
        NonAtomicHeadNode {head_ptr:ptr,head_count:cnt}
    }

    pub unsafe fn get_guard_handle(self) -> Option<&'static Node> {
        self.head_ptr.map(|val|->&Node {
            val.as_ref()
        })
    }
}