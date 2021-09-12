mod tests {
    use loom::thread;

    use gclf_hyaline::retire;
    use gclf_hyaline::pin;

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
    fn collector_test_main() {
        loom::model(|| {
            let first_garb = Box::new(TestNode{foo:1,bar:2});
            let second_garb = Box::new(TestNode{foo:2,bar:2});
            let third_garb = Box::new(TestNode{foo:3,bar:2});
            let fourth_garb = Box::new(TestNode{foo:4,bar:2});
            {
                let _guard = pin();
                retire(first_garb); 
                retire(second_garb);
            }

            let handle = thread::spawn(|| {
                {let _guard = pin();
                retire(third_garb); 
                retire(fourth_garb);}         
            });
            handle.join().unwrap();
        });
    }
}