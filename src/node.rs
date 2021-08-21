use std::ptr::NonNull;
use crate::batch::Batch;
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
    pub list : Option< NonNull<Node> >,
    pub batch : Option< Box<Node> >,
    pub nref_node: Option<NonNull<Batch>>
}

impl Node {
    pub fn new<T:'static>(val:Box<T>) -> Self {
        Node{
            val:Some(Box::new(GarbageNode::new(val))),
            list:None,
            batch:None,
            nref_node:None
        }
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