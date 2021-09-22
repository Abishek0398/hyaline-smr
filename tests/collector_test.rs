use std::sync::Arc;
use std::sync::atomic::Ordering::SeqCst;

use gclf_hyaline::collector::{Collector, Smr};
use loom::thread;
use loom::sync::atomic::AtomicUsize;


struct TestNode {
    foo:i32,
    bar:i32
}
impl Drop for TestNode {
    fn drop(&mut self) {
        println!("Testnode drop woo hoo : {}", self.foo);
    }
}
#[test]
fn collector_test() {
    loom::model(|| {
        let v1 = Arc::new(AtomicUsize::new(0));
        let v2 = v1.clone();
        let v3= v1.clone();

        let loc_coll_1 = Arc::new(Collector::new());
        let loc_coll_2 = loc_coll_1.clone();
        let loc_coll_3 = loc_coll_1.clone();

        let first_garb = Box::new(TestNode{foo:1,bar:2});
        let second_garb = Box::new(TestNode{foo:2,bar:2});
        let third_garb = Box::new(TestNode{foo:3,bar:2});
        let fourth_garb = Box::new(TestNode{foo:4,bar:2});

        let five_garb = Box::new(TestNode{foo:5,bar:2});
        let six_garb = Box::new(TestNode{foo:6,bar:2});

        {let _guard1 = loc_coll_1.pin();
        loc_coll_1.retire(first_garb); 
        loc_coll_1.retire(second_garb);}

        thread::spawn(move|| {
            let _guard2 = loc_coll_2.pin();
            loc_coll_2.retire(third_garb); 
            loc_coll_2.retire(fourth_garb);
            v1.store(1, SeqCst);
        });

        thread::spawn(move|| {
            let _guard3 = loc_coll_3.pin();
            loc_coll_3.retire(five_garb); 
            loc_coll_3.retire(six_garb);
            v2.store(2, SeqCst);
        });

        println!("{}",v3.load(SeqCst));
    });
}