/*
   These tests serve very little purpose for now as the atomicdouble type is a
   custom atomic type and loom doesnt support it. These are here jst to test out loom's
   features and maybe test some permutations using loom::thread.
   Put it short -> This test is useless as of now
*/
#![cfg(loom)]
use std::ptr;
use std::{mem::ManuallyDrop, ptr::NonNull};

use loom::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use loom::sync::Arc;
use loom::thread;

use hyaline_smr::{
    self as hyaline, {Collector, Smr},
};

use lazy_static::lazy_static;

lazy_static! {
    static ref DROP_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
}

struct TestNode {
    foo: i32,
    bar: i32,
}
impl Drop for TestNode {
    fn drop(&mut self) {
        DROP_COUNT.fetch_add(1, atomicdouble::Ordering::Release);
    }
}
#[test]
fn collector_test() {
    loom::model(|| {
        let v1 = Arc::new(AtomicUsize::new(0));
        let v2 = v1.clone();
        let v3 = v1.clone();

        let loc_coll_1 = Arc::new(Collector::new());
        let loc_coll_2 = loc_coll_1.clone();
        let loc_coll_3 = loc_coll_1.clone();

        let first_garb = Box::new(TestNode { foo: 1, bar: 2 });
        let second_garb = Box::new(TestNode { foo: 2, bar: 2 });
        let third_garb = Box::new(TestNode { foo: 3, bar: 2 });
        let fourth_garb = Box::new(TestNode { foo: 4, bar: 2 });

        let five_garb = Box::new(TestNode { foo: 5, bar: 2 });
        let six_garb = Box::new(TestNode { foo: 6, bar: 2 });

        {
            let guard1 = loc_coll_1.pin();
            unsafe {
                loc_coll_1.retire(NonNull::new(Box::into_raw(first_garb)), &guard1);
                loc_coll_1.retire(NonNull::new(Box::into_raw(second_garb)), &guard1);
            }
        }

        thread::spawn(move || {
            let guard2 = loc_coll_2.pin();
            unsafe {
                loc_coll_2.retire(NonNull::new(Box::into_raw(third_garb)), &guard2);
                loc_coll_2.retire(NonNull::new(Box::into_raw(fourth_garb)), &guard2);
            }
            v1.store(1, Ordering::Release);
        });

        thread::spawn(move || {
            let guard3 = loc_coll_3.pin();
            unsafe {
                loc_coll_3.retire(NonNull::new(Box::into_raw(five_garb)), &guard3);
                loc_coll_3.retire(NonNull::new(Box::into_raw(six_garb)), &guard3);
            }
            v2.store(2, Ordering::Release);
        });
        v3.load(Ordering::Acquire);
        while DROP_COUNT.load(atomicdouble::Ordering::Acquire) < 6 {}
        assert_eq!(DROP_COUNT.load(atomicdouble::Ordering::Acquire), 6);
    });
}

#[test]
fn treiber_stack() {
    /// Treiber's lock-free stack.
    ///
    /// Usable with any number of producers and consumers.
    #[derive(Debug)]
    pub struct TreiberStack<T> {
        head: AtomicPtr<Node<T>>,
    }

    #[derive(Debug)]
    struct Node<T> {
        data: ManuallyDrop<T>,
        next: AtomicPtr<Node<T>>,
    }

    impl<T> TreiberStack<T> {
        /// Creates a new, empty stack.
        pub fn new() -> TreiberStack<T> {
            TreiberStack {
                head: AtomicPtr::default(),
            }
        }

        /// Pushes a value on top of the stack.
        pub fn push(&self, t: T) {
            let n = Box::new(Node {
                data: ManuallyDrop::new(t),
                next: AtomicPtr::default(),
            });

            let _guard = hyaline::pin();
            let mut head = self.head.load(Ordering::Relaxed);
            let a_n = Box::into_raw(n);
            loop {
                unsafe {
                    a_n.as_ref().unwrap().next.store(head, Ordering::Relaxed);
                };
                match self
                    .head
                    .compare_exchange(head, a_n, Ordering::Release, Ordering::Relaxed)
                {
                    Ok(_) => break,
                    Err(e) => head = e,
                }
            }
        }

        /// Attempts to pop the top element from the stack.
        ///
        /// Returns `None` if the stack is empty.
        pub fn pop(&self) -> Option<T> {
            let guard = hyaline_smr::pin();
            loop {
                let head = self.head.load(Ordering::Acquire);

                match unsafe { head.as_ref() } {
                    Some(h) => {
                        let next = h.next.load(Ordering::Relaxed);

                        if self
                            .head
                            .compare_exchange(head, next, Ordering::Relaxed, Ordering::Relaxed)
                            .is_ok()
                        {
                            unsafe {
                                hyaline::retire(NonNull::new(head), &guard);
                                return Some(ManuallyDrop::into_inner(ptr::read(&(*h).data)));
                            }
                        }
                    }
                    None => return None,
                }
            }
        }

        /// Returns `true` if the stack is empty.
        pub fn is_empty(&self) -> bool {
            let _guard = hyaline::pin();
            self.head.load(Ordering::Acquire).is_null()
        }
    }

    impl<T> Drop for TreiberStack<T> {
        fn drop(&mut self) {
            while self.pop().is_some() {}
        }
    }

    loom::model(|| {
        let stack1 = Arc::new(TreiberStack::new());
        let stack2 = Arc::clone(&stack1);

        // use 5 since it's greater than the 4 used for the sanitize feature
        let jh = thread::spawn(move || {
            for i in 0..5 {
                stack2.push(i);
                assert!(stack2.pop().is_some());
            }
        });

        for i in 0..5 {
            stack1.push(i);
            assert!(stack1.pop().is_some());
        }

        jh.join().unwrap();
        assert!(stack1.pop().is_none());
        assert!(stack1.is_empty());
    });
}
