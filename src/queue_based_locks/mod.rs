use crate::rcu::Rcu;
use rand::Rng;
use std::{
    cell::UnsafeCell,
    collections::VecDeque,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicBool, AtomicPtr, Ordering::*},
        Arc, Mutex,
    },
    thread::{self, Thread},
    time::{Duration, Instant},
};
use std_reset::{prelude::Default, traits::as_prim::AsPrim};

struct QueueLock<T> {
    queue: Rcu<VecDeque<Thread>>,
    is_busy: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for QueueLock<T> {}

impl<T: Default> QueueLock<T> {
    pub fn new(data: T) -> Self {
        Self {
            queue: Rcu::new(VecDeque::new()),
            data: UnsafeCell::new(data),
            is_busy: AtomicBool::new(false),
        }
    }
}

struct Guard<'a, T> {
    queue_lock: &'a QueueLock<T>,
}
impl<T> Deref for Guard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.queue_lock.data.get() }
    }
}

impl<T> DerefMut for Guard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.queue_lock.data.get() }
    }
}

impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.queue_lock.is_busy.store(false, Release);
        self.queue_lock.queue.change(|queue| {
            queue.pop_front().map(|t| t.unpark());
        });
    }
}
impl<T> QueueLock<T> {
    pub fn lock(&self) -> Guard<T> {
        while self
            .is_busy
            .compare_exchange(false, true, Relaxed, Relaxed)
            .is_err()
        {
            self.queue.change(|queue| {
                queue.push_back(thread::current());
            });
            if self.is_busy.load(Relaxed) {
                thread::park();
                break;
            }
        }
        Guard { queue_lock: self }
    }
    pub fn unlock(&self) {
        self.is_busy.store(false, Release);
        self.queue.change(|queue| {
            let t = queue.pop_front();
            if let Some(t) = t {
                t.unpark();
            };
        });
    }
}

#[test]
fn test() {
    let queue = QueueLock::new(0);

    let times = Mutex::new(Vec::new());
    thread::scope(|s| {
        for _ in 0..10 {
            s.spawn(|| {
                let start = Instant::now();
                thread::scope(|s| {
                    for _ in 0..3_000 {
                        s.spawn({
                            || {
                                queue.lock();
                            }
                        });
                    }
                });
                times.lock().unwrap().push(start.elapsed());
            });
        }
    });
    let times = times.into_inner().unwrap();

    let t = times
        .iter()
        .cloned()
        .reduce(|a, b| a + b)
        .unwrap()
        .as_micros()
        / times.len().as_::<u128>();
    dbg!(Duration::from_micros(t.as_()));
}
