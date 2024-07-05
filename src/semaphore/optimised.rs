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
    counter: AtomicU32,
}

/// Вместо того чтобы блокировать counter на всем пути до разблокировки,
/// мы выполняем операции атомарно солгалсуясь с другими потоками
impl Semaphore {
    pub const fn new(count_of_resurses: u32) -> Self {
        Self {
            counter: AtomicU32::new(count_of_resurses),
        }
    }
    pub fn signal(&self) {
        let mut c = self.counter.load(Relaxed);
        if c == u32::MAX {
            panic!("Счётчик семафора достиг максимального значения");
        }

        while let Err(e) = self.counter.compare_exchange(c, c + 1, Relaxed, Relaxed) {
            c = e
        }
        wake_one(&self.counter);
    }
    pub fn wait(&self) {
        while self.counter.load(Acquire) == 0 {
            wait(&self.counter, 0);
        }
        self.counter.fetch_sub(1, Release);
    }
}

#[test]
fn wait_and_signal() {
    let count_of_resurses = 50_000;
    let mut semaphore = Semaphore::new(count_of_resurses);

    let start = Instant::now();
    thread::scope(|s| {
        for _ in 0..count_of_resurses {
            s.spawn(|| {
                semaphore.wait();
            });
        }
    });
    assert_eq!(*semaphore.counter.get_mut(), 0);
    println!("time: {:?}", start.elapsed(),);
}

// вариации когда может быть блокировка
#[test]
fn blocked_wait() {
    let semaphore: &'static Semaphore = Box::leak(Box::new(Semaphore::new(0)));

    thread::spawn(move || {
        semaphore.wait();
        panic!("Not blocked");
    });
    thread::sleep(std::time::Duration::from_secs(3));
}
#[test]
fn blocked_signal() {
    let semaphore: &'static Semaphore = Box::leak(Box::new(Semaphore::new(0)));
    semaphore.counter.store(u32::MAX, Relaxed);
    thread::spawn(move || {
        semaphore.signal();
        panic!("Not blocked");
    });
    thread::sleep(std::time::Duration::from_secs(3));
}
