use crate::retire_list::Node;
use crate::collector::Collector;

pub struct Guard<'a> {
    active_collector : &'a Collector,
    handle : Option< &'a Node>,
    slot : usize
}

impl<'a> Guard<'a> {
    fn new(coll:&Collector)-> Guard<'_> {
        Guard{active_collector:coll,handle:None,slot:0}
    }
    fn is_handle(&self,check_val : Option< NonNull<Node> >) -> bool {
        self.handle == check_val
    }
}
impl<'a> Drop for Guard<'a>{
    fn drop(&mut self) {
        self.active_collector.unpin(self);
    }
}