//! Safe Memory Reclaimation using Hyaline algorithm                                                                                                                                             
//! [Snapshot-Free, Transparent, and Robust Memory Reclamation for Lock-Free Data Structures][hyaline]
//!
//! An interesting problem concurrent collections deal with comes from the remove operation.
//! Suppose that a thread removes an element from a lock-free map, while another thread is reading
//! that same element at the same time. The first thread must wait until the second thread stops
//! reading the element. Only then it is safe to destruct it.
//!
//! Programming languages that come with garbage collectors solve this problem trivially. The
//! garbage collector will destruct the removed element when no thread can hold a reference to it
//! anymore.
//!
//! This crate implements the Scalable Multiple-List without support for stalled threads
//! version of Hyaline. When an element gets removed from a concurrent collection, it is inserted into a global list of garbage and
//! every time a thread accesses a collection, it registers itself to the colletor.
//! When the thread de-registers from the collector it destructs some garbage that became so old that no thread
//! can be referencing it anymore.
//!
//! That is the general mechanism behind Hyaline memory reclamation, but the details are a bit
//! more complicated. Anyhow, memory reclamation is designed such that the users of concurrent
//! collections don't have to worry much about.
//!
//! Please Refer [Snapshot-Free, Transparent, and Robust Memory
//! Reclamation for Lock-Free Data Structures][hyaline] for futher information.
//!
//! # Pinning
//!
//! Before a concurrent collection can be accessed, a participant must be [`pin`](Collector::pin)ned. By pinning a participant
//! we declare that any object that gets removed from now on must not be destructed just
//! yet.
//!
//! # Garbage
//!
//! Objects that get removed from concurrent collections must be stashed away until all currently
//! pinned participants get unpinned. Such objects can be stored into a thread-local or global
//! storage, where they are kept until the right time for their destruction comes.
//!
//! There is a global shared instance of garbage queue. You can [`retire`](Collector::retire) the garbage values
//! after which the garbage collector will take care of the deallocation of the value at the correct time.
//!
//! # APIs
//!
//! For majority of use cases, just use the default garbage collector by invoking [`pin`] and [`retire`]. If you
//! want to create your own garbage collector, use the [`Collector`] API.
//!
//! # Examples
//! The following is a completely synthetic example.
//! ```
//! use hyaline_smr as hyaline;
//! use lazy_static::lazy_static;
//! use std::{
//!     ptr::NonNull,
//!     sync::atomic::{AtomicUsize, Ordering},
//!     thread,
//! };
//!
//! const MAX_THREADS: usize = 8;
//! lazy_static! {
//!     static ref DROP_COUNT: AtomicUsize = AtomicUsize::new(0);
//! }
//!
//! struct TestNode {
//!    foo: usize
//! }
//!
//! impl Drop for TestNode {
//!     fn drop(&mut self) {
//!         DROP_COUNT.fetch_add(1, Ordering::Relaxed);
//!     }
//! }
//!
//! fn count_drop() {
//!     let mut handle_array = Vec::new();
//!
//!     for _i in 0..MAX_THREADS {
//!         let handle = thread::spawn(move || {
//!             let guard = hyaline::pin();
//!             for j in 0..50 {
//!                 unsafe {
//!                     let x = Box::new(TestNode { foo: j });
//!                     let garb = NonNull::new(Box::into_raw(x));
//!                     hyaline::retire(garb, &guard);
//!                 }
//!             }
//!         });
//!         handle_array.push(handle);
//!     }
//!     while DROP_COUNT.load(Ordering::Relaxed) < MAX_THREADS * 50 {}
//!     assert_eq!(DROP_COUNT.load(Ordering::Relaxed), MAX_THREADS * 50);
//! }
//! ```
//!
//! [hyaline]: https://arxiv.org/pdf/1905.07903.pdf

#![doc(test(
    no_crate_inject,
    attr(
        deny(warnings, rust_2018_idioms),
        allow(dead_code, unused_assignments, unused_variables)
    )
))]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]
#![allow(dead_code)]
#![feature(thread_id_value)]

#[cfg(loom)]
#[allow(unused_imports, dead_code)]
mod primitive {
    pub(crate) mod cell {
        pub(crate) use loom::cell::UnsafeCell;
    }
    pub(crate) mod sync {
        pub(crate) mod atomic {
            pub(crate) use loom::sync::atomic::AtomicPtr;
            pub(crate) use loom::sync::atomic::AtomicUsize;
            pub(crate) use loom::sync::atomic::Ordering;
            pub(crate) fn fence(ord: Ordering) {
                loom::sync::atomic::fence(ord)
            }

            pub(crate) use self::fence as compiler_fence;
        }
        pub(crate) use loom::sync::Arc;
    }
    pub(crate) use loom::lazy_static;
    pub(crate) use loom::thread;
    pub(crate) use loom::thread_local;
}

#[cfg(not(loom))]
#[allow(unused_imports, dead_code)]
mod primitive {
    pub(crate) mod cell {
        #[derive(Debug)]
        #[repr(transparent)]
        pub(crate) struct UnsafeCell<T>(::core::cell::UnsafeCell<T>);

        // loom's UnsafeCell has a slightly different API than the standard library UnsafeCell.
        // Since we want the rest of the code to be agnostic to whether it's running under loom or
        // not, we write this small wrapper that provides the loom-supported API for the standard
        // library UnsafeCell. This is also what the loom documentation recommends:
        // https://github.com/tokio-rs/loom#handling-loom-api-differences
        impl<T> UnsafeCell<T> {
            #[inline]
            pub(crate) fn new(data: T) -> UnsafeCell<T> {
                UnsafeCell(::core::cell::UnsafeCell::new(data))
            }

            #[inline]
            pub(crate) fn with<R>(&self, f: impl FnOnce(*const T) -> R) -> R {
                f(self.0.get())
            }

            #[inline]
            pub(crate) fn with_mut<R>(&self, f: impl FnOnce(*mut T) -> R) -> R {
                f(self.0.get())
            }
        }
    }
    pub(crate) mod sync {
        pub(crate) mod atomic {
            pub(crate) use core::sync::atomic::compiler_fence;
            pub(crate) use core::sync::atomic::fence;
            pub(crate) use core::sync::atomic::AtomicPtr;
            pub(crate) use core::sync::atomic::AtomicUsize;
            pub(crate) use core::sync::atomic::Ordering;
        }
        pub(crate) use std::sync::Arc;
    }

    pub(crate) use std::thread;

    pub(crate) use std::thread_local;

    pub(crate) use lazy_static::lazy_static;
}

mod batch;

mod collector;
pub use self::collector::{Collector, Smr};

mod deferred;

mod guard;
pub use self::guard::Guard;

mod headnode;
mod node;

mod default;
pub use self::default::{default_collector, pin, retire};
