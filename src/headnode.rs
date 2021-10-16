use std::ptr::NonNull;

use atomicdouble::AtomicDouble;
use atomicdouble::Ordering;

use crate::collector::ADJS;
use crate::guard::Guard;
use crate::node::Node;

#[derive(Debug)]
pub(crate) struct HeadNode {
    head: AtomicDouble<NonAtomicHeadNode>,
}

impl HeadNode {
    pub(crate) fn new(head_ptr: Option<NonNull<Node>>, head_count: usize) -> Self {
        HeadNode {
            head: AtomicDouble::new(NonAtomicHeadNode {
                head_ptr,
                head_count,
            }),
        }
    }

    pub(crate) fn add_to_slot(&self, val: &mut Node) -> Result<(), ()> {
        let mut curr_node = self.head.load(Ordering::Relaxed);
        loop {
            if curr_node.head_count == 0 {
                val.set_list(None);
                return Err(());
            }
            val.set_list(curr_node.head_ptr);

            let new_node = NonAtomicHeadNode {
                head_ptr: NonNull::new(val),
                head_count: curr_node.head_count,
            };

            //Release because we modify val and we need it to be reflected rit when
            //we switch headnode
            let cxchg_result = self.head.compare_exchange(
                curr_node,
                new_node,
                Ordering::Release,
                Ordering::Relaxed,
            );
            match cxchg_result {
                Ok(_) => {
                    unsafe {
                        Node::add_adjs(curr_node.head_ptr, curr_node.head_count + ADJS);
                    };
                    return Ok(());
                }
                Err(pres_node) => curr_node = pres_node,
            };
        }
    }

    pub(crate) fn pin_slot(&self) -> Option<&'static Node> {
        unsafe {
            // Safe because the headptr obtained from NonAtomicHeadNode returned
            // from fetchadd is either valid(The algorithm ensures this) or None
            self.fetch_add(None, 1, Ordering::Relaxed)
                .get_guard_handle()
        }
    }

    pub(crate) fn unpin_slot(&self, local_guard: &Guard<'_>) {
        let mut curr_head: NonAtomicHeadNode = self.head.load(Ordering::Relaxed);
        loop {
            let mut traverse_node = None;
            let cas_node = {
                if curr_head.head_count == 1 {
                    NonAtomicHeadNode::new(None, 0)
                } else {
                    NonAtomicHeadNode::new(curr_head.head_ptr, curr_head.head_count.wrapping_sub(1))
                }
            };

            if !local_guard.is_handle(curr_head.head_ptr) {
                traverse_node = curr_head
                    .head_ptr
                    .and_then(|val| unsafe { val.as_ref().get_list() });
            }
            match self.head.compare_exchange(
                curr_head,
                cas_node,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    if curr_head.head_count == 1 && curr_head.head_ptr != None {
                        unsafe {
                            Node::add_adjs(curr_head.head_ptr, ADJS);
                        };
                    }
                    if let Some(act_traverse_node) = traverse_node {
                        unsafe {
                            act_traverse_node.as_ref().traverse(local_guard);
                        };
                    }
                    break;
                }
                Err(pres_head) => curr_head = pres_head,
            }
        }
    }

    fn fetch_add(
        &self,
        head_ptr: Option<NonNull<Node>>,
        head_cnt: usize,
        order: Ordering,
    ) -> NonAtomicHeadNode {
        let add_node = NonAtomicHeadNode::new(head_ptr, head_cnt);
        self.head.fetch_add(add_node, order)
    }
}

impl Default for HeadNode {
    fn default() -> Self {
        Self {
            head: Default::default(),
        }
    }
}

#[derive(Debug)]
struct NonAtomicHeadNode {
    head_ptr: Option<NonNull<Node>>,
    head_count: usize,
}

/*
Grossly unsafe but our implementation guarantees that there is no data race
or UB of any kind when sending this type
*/
unsafe impl Send for NonAtomicHeadNode {}

impl Copy for NonAtomicHeadNode {}

impl Clone for NonAtomicHeadNode {
    fn clone(&self) -> Self {
        *self
    }
}

impl Default for NonAtomicHeadNode {
    fn default() -> Self {
        NonAtomicHeadNode {
            head_ptr: None,
            head_count: 0,
        }
    }
}

impl NonAtomicHeadNode {
    pub(crate) fn new(ptr: Option<NonNull<Node>>, cnt: usize) -> Self {
        NonAtomicHeadNode {
            head_ptr: ptr,
            head_count: cnt,
        }
    }

    pub(crate) unsafe fn get_guard_handle(self) -> Option<&'static Node> {
        self.head_ptr.map(|val| -> &Node { val.as_ref() })
    }
}
