use std::ptr::NonNull;

use crate::node::Node;
use crate::collector::Collector;

pub struct Guard<'a,const N:usize> {
    active_collector : &'a Collector<N>,
    handle : Option< &'a Node>,
    slot : usize
}

impl<'a,const N:usize> Guard<'a,N> {
    fn new(coll:&Collector<N>)-> Guard<'_,N> {
        Guard{active_collector:coll,handle:None,slot:0}
    }
    fn is_handle(&self,check_val : Option< NonNull<Node> >) -> bool {
        self.handle == check_val
    }
}
impl<'a,const N:usize> Drop for Guard<'a,N>{
    fn drop(&mut self) {
        self.active_collector.unpin(self);
    }
}