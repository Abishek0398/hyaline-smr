use std::ptr::NonNull;

use crate::node::Node;
use crate::collector::Collector;
use crate::collector::Smr;

pub struct Guard<'a> {
    active_collector : &'a Collector,
    pub(crate) handle : Option< &'a Node>,
    pub(crate) slot : usize
}

impl<'a> Guard<'a> {
    pub fn new(coll:&Collector)-> Guard<'_> {
        Guard{active_collector:coll,handle:None,slot:0}
    }
    
    pub fn is_handle(&self,check_val : Option< NonNull<Node> >) -> bool {

        let first = self.handle.and_then(|val| {
            Some(val as *const Node)
        });

        let second = check_val.and_then(|val| {
            unsafe {
                Some(val.as_ref() as *const Node)
            }
        });

        first == second
    }

}
impl<'a>Drop for Guard<'a>{
    fn drop(&mut self) {
        self.active_collector.unpin(self);
    }
}