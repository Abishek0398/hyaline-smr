//! The default garbage collector.

use std::ptr::NonNull;

use crate::collector::{Collector, Smr};
use crate::guard::Guard;
use crate::primitive::lazy_static;

lazy_static! {
    /// The global default garbage collector.
    static ref COLLECTOR: Collector = Collector::new();
}

/// Pins the current thread.
#[inline]
pub fn pin() -> Guard<'static> {
    COLLECTOR.pin()
}

/// Collects the garbage values form the user. The local_guard argument is just here
/// for ensuring that retire() is called after a pin().
///
/// # Safety
/// Caller must ensure that only logically deleted values of the concerned data structure is
/// provided to the retire method. For example: In a lock-free linkedlist retire() needs to be called
/// only after the concerned node is removed form the list.
#[inline]
pub unsafe fn retire<T>(garbage: Option<NonNull<T>>, local_guard: &Guard<'_>) {
    COLLECTOR.retire(garbage, local_guard);
}

/// Returns the default global collector.
pub fn default_collector() -> &'static Collector {
    &COLLECTOR
}

#[cfg(all(test, not(loom)))]
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
