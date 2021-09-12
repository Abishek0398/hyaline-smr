#[cfg(loom)]
pub(crate) use  {
    loom::sync::atomic::AtomicUsize,
    loom::sync::atomic::Ordering,
    loom::lazy_static,
    loom::thread_local,
};

#[cfg(not(loom))]
pub(crate) use  {
    std::sync::atomic::AtomicUsize,
    std::sync::atomic::Ordering,
    lazy_static::lazy_static,
    std::thread_local,
};