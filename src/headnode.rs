use std::ptr::NonNull;

pub(crate) struct HeadNode {
    head_ptr : Option< NonNull<Node> >,
    head_count : usize
}

/*
Grossly unsafe but our implementation guarantees that there is no data race
or UB of any kind when sending this type
*/
unsafe impl Send for HeadNode{}

impl Copy for HeadNode {}

impl Clone for HeadNode {
    fn Clone(&self) -> Self {
        *self
    }
}

impl Default for HeadNode {
    fn default()->Self {
        HeadNode {head_ptr:None,head_count:0}
    }
}

impl HeadNode {
    fn new(ptr:Option< NonNull<Node> >,cnt: usize) {
        HeadNode {head_ptr:ptr,head_count:cnt}
    }
}