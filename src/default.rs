//! The default garbage collector.

use crate::collector::{Collector, Smr};
use crate::guard::Guard;
use lazy_static::lazy_static;

lazy_static! {
    /// The global default garbage collector.
    static ref COLLECTOR: Collector = Collector::new();
}

/// Pins the current thread.
#[inline]
pub fn pin() -> Guard<'static> {
    COLLECTOR.pin()
}

///Retires a node from the data structure. 
///No new threads should be able to access the retired node after retiring
#[inline]
pub fn retire<T:'static>(garbage:Box<T>) {
    COLLECTOR.retire(garbage);
}


/// Returns the default global collector.
pub fn default_collector() -> &'static Collector {
    &COLLECTOR
}

#[cfg(test)]
mod tests {
    use std::thread;

    
    #[test]
    fn pin_while_exiting() {
        struct Foo;

        impl Drop for Foo {
            fn drop(&mut self) {
                // Pin after `HANDLE` has been dropped. This must not panic.
                super::pin();
            }
        }

        thread_local! {
            static FOO: Foo = Foo;
        }

        let handle = thread::spawn(|| {
            // Initialize `FOO` and then `HANDLE`.
            FOO.with(|_| ());
            super::pin();
            // At thread exit, `HANDLE` gets dropped first and `FOO` second.
        });
        handle.join().unwrap();
    }
}