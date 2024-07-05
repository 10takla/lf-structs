use atomic_wait::{wait, wake_one};
use std::{
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering::*},
        Condvar, Mutex,
    },
    thread,
    time::Instant,
};

pub struct Semaphore {
    pub counter: AtomicU32,
}

/// Вместо того чтобы 
impl Semaphore {
    pub const fn new() -> Self {
        Self {
            counter: AtomicU32::new(0),
        }
    }
    pub fn signal(&self) {
        while self.counter.load(Acquire) == u32::MAX {
            wait(&self.counter, u32::MAX);
        }
        self.counter.fetch_add(1, Release);
        wake_one(&self.counter);
    }
    pub fn wait(&self) {
        while self.counter.load(Acquire) == 0 {
            wait(&self.counter, 0);
        }
        self.counter.fetch_sub(1, Release);
        wake_one(&self.counter);
    }
}

#[test]
fn semaphore() {
    let semaphore = Semaphore::new();

    let start = Instant::now();
    thread::scope(|s| {
        s.spawn(|| {
            for _ in 0..4_000_000 {
                semaphore.signal();
            }
        });

        for _ in 0..4_000_000 {
            semaphore.wait();
        }
    });
    println!(
        "time: {:?}, counter:{:?}",
        start.elapsed(),
        semaphore.counter
    );
}

// вариации когда может быть блокировка
#[test]
fn blocked_wait() {
    let semaphore: &'static Semaphore = Box::leak(Box::new(Semaphore::new()));

    thread::spawn(move || {
        semaphore.wait();
        panic!("Not blocked");
    });
    thread::sleep(std::time::Duration::from_secs(3));
}
#[test]
fn blocked_signal() {
    let semaphore: &'static Semaphore = Box::leak(Box::new(Semaphore::new()));
    semaphore.counter.store(u32::MAX, Relaxed);
    thread::spawn(move || {
        semaphore.signal();
        panic!("Not blocked");
    });
    thread::sleep(std::time::Duration::from_secs(3));
}
