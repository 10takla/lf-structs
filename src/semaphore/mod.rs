use std::{
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering::*},
        Condvar, Mutex,
    },
    thread,
    time::Instant,
};

pub mod optimised;

pub struct Semaphore {
    counter: Mutex<u32>,
    is_wait: Condvar,
}

impl Semaphore {
    pub const fn new(count_of_resurses: u32) -> Self {
        Self {
            counter: Mutex::new(count_of_resurses),
            is_wait: Condvar::new(),
        }
    }
    pub fn signal(&self) {
        let mut counter = self.counter.lock().unwrap();
        *counter += 1;
        self.is_wait.notify_one();
    }
    pub fn wait(&self) {
        let mut counter = self.counter.lock().unwrap();
        if *counter == 0 {
            counter = self.is_wait.wait(counter).unwrap();
        }
        *counter -= 1;
    }
}

#[test]
fn wait_and_signal() {
    let count_of_resurses = 50_000;
    let semaphore = Semaphore::new(count_of_resurses);

    let start = Instant::now();
    thread::scope(|s| {
        for _ in 0..count_of_resurses {
            s.spawn(|| {
                semaphore.wait();
            });
        }
    });
    assert_eq!(*semaphore.counter.lock().unwrap(), 0);
    println!("time: {:?}", start.elapsed(),);
}
