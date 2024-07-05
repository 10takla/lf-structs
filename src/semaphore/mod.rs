use std::{
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering::*},
        Condvar, Mutex,
    },
    thread,
    time::Instant,
};
use test::Bencher;

pub mod optimised;

pub struct Semaphore {
    counter: Mutex<u32>,
    is_wait: Condvar,
}

impl Semaphore {
    pub const fn new() -> Self {
        Self {
            counter: Mutex::new(0),
            is_wait: Condvar::new(),
        }
    }
    pub fn signal(&self) {
        let mut counter = self.counter.lock().unwrap();
        // не нужен while (перепроверка) так как counter блокирует всех и никто не может изменить значение во время wait
        if *counter == u32::MAX {
            counter = self.is_wait.wait(counter).unwrap();
        }
        *counter += 1;
        self.is_wait.notify_one();
    }
    pub fn wait(&self) {
        let mut counter = self.counter.lock().unwrap();
        // не нужен while (перепроверка) так как counter блокирует всех и никто не может изменить значение во время wait
        if *counter == 0 {
            counter = self.is_wait.wait(counter).unwrap();
        }
        *counter -= 1;
        self.is_wait.notify_one();
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
    println!("time: {:?} counter:{}", start.elapsed(), semaphore.counter.lock().unwrap());
}


#[bench]
fn semaphore_benchmark(b: &mut Bencher) {
    let semaphore = Semaphore::new();

    b.iter(|| {
        for _ in 0..4_000_000 {
            semaphore.signal();
        }
        for _ in 0..4_000_000 {
            semaphore.wait();
        }
    });

    // Optionally, print or assert results if needed
    println!("counter: {:?}", semaphore.counter);
}

#[test]
fn counter_overflow() {
    let semaphore = Semaphore::new();
    *semaphore.counter.lock().unwrap() = u32::MAX;
    
    thread::scope(|s| {
        s.spawn(|| {
            thread::sleep(std::time::Duration::from_millis(1000));
            // освобождает уменьшая counter до u32::MAX - 1
            semaphore.wait();
        });
        // блокирует пока counter переполнен
        semaphore.signal();
    });
}
