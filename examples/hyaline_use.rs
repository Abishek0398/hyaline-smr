use std::sync::atomic::{AtomicPtr, AtomicUsize};
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use gclf_hyaline::collector::{Collector, Smr};
use lazy_static::lazy_static;

use rand::Rng;

lazy_static! {
    static ref COLLECTOR: Collector = Collector::new();
}

fn worker(a: Arc<AtomicPtr<AtomicUsize>>) -> usize {
    let mut rng = rand::thread_rng();
    let mut sum = 0;

    if rng.gen() {
        thread::sleep(Duration::from_millis(1));
    }
    let timeout = Duration::from_millis(rng.gen_range(0..10));
    let now = Instant::now();

    while now.elapsed() < timeout {
        for _ in 0..100 {
            let _guard = COLLECTOR.pin();

            let val = if rng.gen() {
                let t = Box::new(AtomicUsize::new(sum));
                let p = a.swap(Box::into_raw(t), AcqRel);
                unsafe {
                    COLLECTOR.retire(Box::from_raw(p));
                    if let Some(act_p) = p.as_ref() {
                        act_p.load(Relaxed)
                    }
                    else {
                        0
                    }
                }   
            } else {
                let p = a.load(Acquire);
                unsafe {
                    if let Some(act_p) = p.as_ref() {
                        act_p.fetch_add(sum,Relaxed)
                    }
                    else {
                        0
                    }
                }
            };
            sum = sum.wrapping_add(val);
        }
    }

    sum
}

fn main() {
    for _ in 0..100 {
        let temp = Box::new(AtomicUsize::new(777));
        let a = Arc::new(AtomicPtr::new(Box::into_raw(temp)));

        let threads = (0..16)
            .map(|_| {
                let a = a.clone();
                thread::spawn(move || worker(a))
            })
            .collect::<Vec<_>>();

        for t in threads {
            t.join().unwrap();
        }
        let new_temp = Box::new(AtomicUsize::new(777));
        
        a.swap(Box::into_raw(new_temp), AcqRel);
    }
}