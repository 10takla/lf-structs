use std::cell::RefCell;
use std::fmt::{Debug, Display};
use std::ops::DerefMut;
use std::sync::atomic::{AtomicU32, Ordering::*};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use std::{ops::Deref, sync::atomic::AtomicPtr};
use std_reset::prelude::Display;

#[derive(Debug)]
pub struct Rcu<T> {
    ptr: AtomicPtr<T>,
}

impl<T: Display + Debug> Display for Rcu<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rcu")
            .field("ptr", &unsafe { self.ptr.load(Relaxed).as_ref() })
            .finish()
    }
}

impl<T: Clone> Clone for Rcu<T> {
    fn clone(&self) -> Self {
        Self {
            ptr: AtomicPtr::new(
                if let Some(t) = unsafe { self.ptr.load(Relaxed).as_ref() } {
                    Box::into_raw(Box::new(t.clone()))
                } else {
                    std::ptr::null_mut()
                },
            ),
        }
    }
}

impl<T> Deref for Rcu<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr.load(Relaxed) }
    }
}
// Для неатомарного измения данных
// impl<T> DerefMut for Rcu<T> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         unsafe { &mut *self.ptr.load(Relaxed) }
//     }
// }
impl<T> Rcu<T> {
    pub fn new(data: T) -> Self {
        Self {
            ptr: AtomicPtr::new(Box::into_raw(Box::new(data))),
        }
    }
}
impl<T: Clone> Rcu<T> {
    pub fn load(&self) -> T {
        unsafe { &*self.ptr.load(Acquire) }.clone()
    }
    /// Изменяет данные на которые ссылается [`AtomicPtr`] атомарно
    pub fn change(&self, f: impl Fn(&mut T)) {
        let mut load_data = self.ptr.load(Acquire);

        // Если параллельный поток изменил данные типа между `.load(Acquire)` и последующим измененнием и сохранением данных,
        // то необходимо перевыполнить заново этот процесс с
        // новыми данными (который дал другой параллельный поток),
        // до тех пор пока другой поток не сможет перебить текущий в гонке данных
        loop {
            let mut changed_data = unsafe { &*load_data }.clone();
            f(&mut changed_data);
            let new_ptr = Box::into_raw(Box::new(changed_data));
            match self.ptr.compare_exchange(
                load_data,
                new_ptr,
                Release,
                Relaxed,
            ) {
                Ok(load_data) => {
                    unsafe {
                        Box::from_raw(load_data);
                    }
                    break;
                }
                // если данные были изменены (параллельным потоком), то выполняем цикл с новыми данными
                Err(e) => {
                    load_data = e;
                    unsafe {
                        Box::from_raw(new_ptr);
                    }
                },
            }
        }
    }
}


#[test]
fn test() {
    #[derive(Clone, Debug, Display)]
    struct User {
        name: String,
        password: String,
    }

    let rcu = &Rcu::new(User {
        name: String::from("name"),
        password: String::from("password"),
    });
    let n = 10_000;
    thread::scope(|s| {
        for i in 0..n {
            s.spawn(move || {
                rcu.change(|user| {
                    user.name = format!("new_name {i}");
                });
            });
        }
        for i in 0..n {
            s.spawn({
                move || {
                    rcu.change(|user| {
                        user.password = format!("new_password {i}");
                    });
                }
            });
        }
    });
    println!("{:?}", rcu.load());
}

#[test]
fn counter() {
    #[derive(Clone, Debug, Display)]
    struct Counter {
        count: u32,
    }
    let rcu = &Rcu::new(Counter { count: 0 });

    let start = Instant::now();
    thread::scope(|s| {
        for _ in 0..10_000 {
            s.spawn(move || {
                rcu.change(|counter| {
                    counter.count += 1;
                });
            });
        }
    });
    println!("{} {:?}", rcu.load(), start.elapsed());

    let counter = Arc::new(Mutex::new(Counter { count: 0 }));
    let start = Instant::now();
    thread::scope(|s| {
        for _ in 0..10_000 {
            let rcu = Arc::clone(&counter);
            s.spawn(move || {
                let mut counter = rcu.lock().unwrap();
                counter.count += 1;
            });
        }
    });
    println!("{} {:?}", counter.lock().unwrap(), start.elapsed());

    let counter = AtomicU32::new(1);
    let start = Instant::now();
    thread::scope(|s| {
        for _ in 1..=9 {
            s.spawn(|| {
                let c = counter.load(Relaxed);
                thread::sleep(std::time::Duration::from_secs(1));
                counter.fetch_add(c, Relaxed);
            });
        }
    });
    println!("{:?} {:?}", counter, start.elapsed());
}


#[test]
fn pref() {
    const N_THREADS: usize = 4;
    const N_ITERS: usize = 1000000;

    // Тестирование с использованием RCU
    let rcu = Arc::new(Rcu::new(0usize));
    let rcu_start = Instant::now();

    let mut rcu_handles = vec![];
    for _ in 0..N_THREADS {
        let rcu = Arc::clone(&rcu);
        
        rcu_handles.push(thread::spawn(move || {
            for _ in 0..N_ITERS {
                rcu.change(|data| {
                    *data += 1;
                });
            }
        }));
    }

    for handle in rcu_handles {
        handle.join().unwrap();
    }

    let rcu_duration = rcu_start.elapsed();
    let rcu_result = rcu.load();

    // Тестирование с использованием Mutex
    let mutex_data = Arc::new(Mutex::new(0usize));
    let mutex_start = Instant::now();

    let mut mutex_handles = vec![];
    for _ in 0..N_THREADS {
        let mutex_data = Arc::clone(&mutex_data);
        mutex_handles.push(thread::spawn(move || {
            for _ in 0..N_ITERS {
                let mut data = mutex_data.lock().unwrap();
                *data += 1;
            }
        }));
    }

    for handle in mutex_handles {
        handle.join().unwrap();
    }

    let mutex_duration = mutex_start.elapsed();
    let mutex_result = *mutex_data.lock().unwrap();

    println!("RCU Result: {}, Duration: {:?}", rcu_result, rcu_duration);
    println!("Mutex Result: {}, Duration: {:?}", mutex_result, mutex_duration);
}