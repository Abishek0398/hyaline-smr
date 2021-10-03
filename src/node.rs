use std::ptr::NonNull;
use crate::{batch::Batch, guard::Guard};
use std::sync::atomic::Ordering;
use educe::Educe;

/*
This is the type that will be used in local batches and retirement lists. 
We make the batch link as box because they own the Node and all 
the drops will happen through that link.
*/

#[derive(Educe)]
#[educe(Debug)]
pub struct Node {
    #[educe(Debug(ignore))]
    val: Option< Box<dyn Garbage> >,
    list : Option< NonNull<Node> >,
    batch : Option< Box<Node> >,
    nref_node: Option<NonNull<Batch>>
}

impl Node {
    pub(crate) fn new<T:'static>(val:Box<T>) -> Self {
        Node{
            val:Some(Box::new(GarbageNode::new(val))),
            list:None,
            batch:None,
            nref_node:None
        }
    }

    pub(crate) fn set_list(&mut self,list:Option<NonNull<Node>>) {
        self.list = list;
    }

    pub(crate) fn set_batch(&mut self,batch:Option<Box<Node>>) {
        self.batch = batch;
    }

    pub(crate) fn set_nref_node(&mut self,nref_node:Option<NonNull<Batch>>) {
        self.nref_node = nref_node;
    }

    pub(crate) unsafe fn fetch_add_nref(&self,val:usize,ordering:Ordering)->usize{
        self.nref_node.unwrap()
        .as_ref()
        .nref
        .fetch_add(val,ordering)
    }

    pub(crate) unsafe fn traverse(&self, local_guard:&Guard) {
        let mut current = self.list;
        loop {
            match current {
                Some(_) => {},
                None => {
                    break;
                },
            };
            if let None = current.unwrap().as_ref().nref_node {
                println!("HIIIIIIIIIIIIIIIIIIIIIIISDFJDFDJF");
                let x = current.unwrap().as_ref();
                let _y = x.get_batch_ptr();
            }
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

    pub(crate) unsafe fn add_adjs(node:Option<NonNull<Node>>, val:usize) {
        match node {
            Some(node_val) => {
                let prev_val = node_val.as_ref().fetch_add_nref(val, Ordering::SeqCst);
                if prev_val.wrapping_add(val) == 0 {
                    let _ = Box::from_raw(node_val.as_ref().get_batch_ptr());
                }
            },
            None => {},
        };
    }

    pub(crate) fn produce_nodes_filler(&mut self)-> Option<NonNull<Node>> {
        match self.batch.as_mut() {
            Some(val) => {
                NonNull::new(val.as_mut() as *mut Node)
            },

            None => {
                let mut batch_filler = Box::new(Node::default());
                let return_val = NonNull::new(batch_filler.as_mut() as *mut Node);
                batch_filler.nref_node = self.nref_node;
                self.batch = Some(batch_filler);
                return_val
            }
        }
    }

    pub(crate) unsafe fn get_batch_ptr(&self)->*mut Batch {
        self.nref_node
        .unwrap()
        .as_ptr()
    }
}

impl Default for Node {
    fn default() -> Self {
        Node{
            val:None,
            list:None,
            batch:None,
            nref_node:None
        }
    }
}

/*
This is just a wrapper around the original garbage. This is done to achieve
runtime polymorphism. One thing to note here is that, since GarbageNode<T>
needs Box<T> any node that is passed to the GC has to be of type Box<T>.
This will be explained in detail in the function which will be used to
retire nodes.
*/
struct GarbageNode<T> {
    act_garbage : Box<T>
}

impl<T> GarbageNode<T> {
    //creates a new GarbageNode<T>.
    #[inline]
    fn new(input:Box<T>) -> Self{
        GarbageNode{act_garbage:input}
    }
}

/*
TODO:FIND WAYS TO AVOID THIS AND STILL ACHIEVE POLYMORPHISM.
*/
trait Garbage {
}

impl<T> Garbage for GarbageNode<T> {}

#[cfg(test)]
mod tests {
    use crate::node::Node;
    #[test]
    fn create_drop_test() {
        let _y = Node::new(Box::new(5));
    }
}