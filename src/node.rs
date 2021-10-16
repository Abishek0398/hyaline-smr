use std::{mem, ptr::NonNull};

use crate::primitive::sync::atomic::Ordering;
use crate::{batch::Batch, deferred::Deferred, guard::Guard};

/*
This is the type that will be used in local batches and retirement lists.
We make the batch link as box because they own the Node and all
the drops will happen through that link.
*/

#[derive(Debug)]
pub(crate) struct Node {
    val: Deferred,
    list: Option<NonNull<Node>>,
    batch: Option<Box<Node>>,
    nref_node: Option<NonNull<Batch>>,
}

impl Node {
    pub(crate) fn new<T>(val: Box<T>) -> Self {
        Node {
            val: Deferred::new(move || drop(val)),
            list: None,
            batch: None,
            nref_node: None,
        }
    }

    pub(crate) fn get_list(&self) -> Option<NonNull<Node>> {
        self.list
    }

    pub(crate) fn set_list(&mut self, list: Option<NonNull<Node>>) {
        self.list = list;
    }

    pub(crate) fn set_batch(&mut self, batch: Option<Box<Node>>) {
        self.batch = batch;
    }

    pub(crate) fn set_nref_node(&mut self, nref_node: Option<NonNull<Batch>>) {
        self.nref_node = nref_node;
    }

    pub(crate) fn is_present_batch_ptr(&self) -> bool {
        self.batch.is_some()
    }

    //unsafe because its up to the caller to make sure the nref_node is valid
    pub(crate) unsafe fn fetch_add_nref(&self, val: usize, ordering: Ordering) -> usize {
        self.nref_node
            .unwrap()
            .as_ref()
            .fetch_add_nref(val, ordering)
    }

    pub(crate) unsafe fn traverse(&self, local_guard: &Guard<'_>) {
        let mut current = Some(NonNull::from(self));
        loop {
            let next = current.unwrap().as_ref().list;
            let prev_val = current
                .unwrap()
                .as_ref()
                .nref_node
                .unwrap()
                .as_ref()
                .fetch_sub_nref(1, Ordering::AcqRel);
            if prev_val.wrapping_sub(1) == 0 {
                let _ = Box::from_raw(current.unwrap().as_ref().nref_node.unwrap().as_ptr());
            }
            if next.is_none() || local_guard.is_handle(current) {
                break;
            }
            current = next;
        }
    }

    pub(crate) unsafe fn add_adjs(node: Option<NonNull<Node>>, val: usize) {
        if let Some(node_val) = node {
            let prev_val = node_val.as_ref().fetch_add_nref(val, Ordering::AcqRel);
            if prev_val.wrapping_add(val) == 0 {
                let _ = Box::from_raw(node_val.as_ref().get_batch_ptr());
            }
        }
    }

    pub(crate) fn produce_nodes_filler(&mut self) -> Option<NonNull<Node>> {
        match self.batch.as_ref() {
            Some(val) => Some(NonNull::from(val.as_ref())),

            None => {
                let mut batch_filler = Box::new(Node::default());
                let return_val = NonNull::new(batch_filler.as_mut() as *mut Node);
                batch_filler.nref_node = self.nref_node;
                self.batch = Some(batch_filler);
                return_val
            }
        }
    }

    pub(crate) fn get_batch_ptr(&self) -> *mut Batch {
        self.nref_node.unwrap().as_ptr()
    }
}

fn no_op_func() {}

impl Default for Node {
    fn default() -> Self {
        Node {
            val: Deferred::new(no_op_func),
            list: None,
            batch: None,
            nref_node: None,
        }
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let no_op = Deferred::new(no_op_func);
        let owned_deferred = mem::replace(&mut self.val, no_op);
        owned_deferred.call();
    }
}
