use std::ptr::NonNull;

use crate::collector::Collector;
use crate::collector::Smr;
use crate::node::Node;

/// A RAII guard which keeps the thread active in garbage collection.
/// The thread will be unpinned automatically upon guard's destruction
/// # Multiple guards
///
/// Pinning is reentrant and it is perfectly legal to create multiple guards. But it is not advised
/// to have a gurad while another guard is still active for a thread. This leads to unnecessary
/// pinning of the thread again.
/// ```
/// use hyaline;
///
/// let guard1 = hyaline::pin();
/// // This kind of usage is not advised as it incurs some performance penalty
/// let guard2 = hyaline::pin();
///
/// hyaline::retire(some_val);
pub struct Guard<'a> {
    active_collector: &'a Collector,
    pub(crate) handle: Option<&'a Node>,
    pub(crate) slot: usize,
}

impl<'a> Guard<'a> {
    pub(crate) fn new(coll: &Collector) -> Guard<'_> {
        Guard {
            active_collector: coll,
            handle: None,
            slot: 0,
        }
    }

    pub(crate) fn is_handle(&self, check_val: Option<NonNull<Node>>) -> bool {
        let first = self.handle.map(|val| val as *const Node);

        let second = check_val.map(|val| val.as_ptr() as *const Node);

        first == second
    }
}
impl<'a> Drop for Guard<'a> {
    fn drop(&mut self) {
        self.active_collector.unpin(self);
    }
}
