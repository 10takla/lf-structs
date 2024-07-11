use super::*;
use std::{
    sync::atomic::{AtomicBool, AtomicU64},
    time::Duration,
};
use std_reset::{prelude::Deref, traits::as_prim::AsPrim};

#[derive(Deref)]
pub struct RcuGC<T> {
    #[deref]
    rcu: Rcu<T>,
    is_used: AtomicU64,
    garbage_collector: Mutex<Vec<*mut T>>,
}

unsafe impl<T> Sync for RcuGC<T> where T: Send {}
unsafe impl<T> Send for RcuGC<T> where T: Send {}

impl<T: Clone> RcuGC<T> {
    pub fn new(data: T) -> Self {
        Self {
            rcu: Rcu::new(data),
            garbage_collector: Mutex::new(Vec::new()),
            is_used: AtomicU64::new(0),
        }
    }
    pub fn load(&self) -> T {
        self.rcu.load()
    }
    pub fn change(&self, f: impl Fn(&mut T)) {
        let mut load_data = self.ptr.load(Acquire);
        self.is_used.fetch_add(1, Relaxed);
        loop {
            let mut changed_data = unsafe { &mut *load_data }.clone();
            f(&mut changed_data);
            let new_ptr = Box::into_raw(Box::new(changed_data));
            match self
                .ptr
                .compare_exchange(load_data, new_ptr, AcqRel, Relaxed)
            {
                Ok(load_data) => {
                    // если garbage_collector заблокирован, то это значит происходит очистка старых указателей
                    self.garbage_collector.lock().unwrap().push(load_data);
                    self.is_used.fetch_sub(1, Relaxed);
                    break;
                }
                Err(e) => {
                    load_data = e;
                    unsafe {
                        Box::from_raw(new_ptr);
                    }
                }
            }
        }
        // Если нет потоков которые еще не завершили цикл изменения данных,
        // то значит старые указатели уже никто не использует, и их можно удалить
        // В любом случае самый последний поток, сможет очистить все указатели,
        //если никто из предыдущих этого не сделал
        if self.is_used.load(Relaxed) == 0 {
            let mut y = self.garbage_collector.lock().unwrap();
            while let Some(ptr) = y.pop() {
                drop(unsafe { Box::from_raw(ptr) });
            }
        }
    }
}

#[test]
fn check_garabage_collector() {
    let rcu = RcuGC::new(0);

    thread::scope(|s| {
        for _ in 0..1_000 {
            s.spawn({
                || {
                    for _ in 0..1_000 {
                        rcu.change(|data| {
                            *data += 1;
                        });
                    }
                }
            });
        }
    });
    assert_eq!(rcu.load(), 1_000_000);
}



#[macro_export]
macro_rules! check {
    ($($code:tt)*) => {
        {
            use memory_stats::memory_stats;

            let start_time = Instant::now();
            let start_lazy_ptr_mem = memory_stats().unwrap().physical_mem;
            // не важно что код в блоке {}, т.к. мы считываем разницу
            // в утечке памяти из-зи указателей без времени жизни
            let general_mem = {
                let start_general_mem = memory_stats().unwrap().physical_mem;
                $($code)*
                memory_stats().unwrap().physical_mem - start_general_mem
            };
            let lazy_ptr_mem = memory_stats().unwrap().physical_mem - start_lazy_ptr_mem;
            
            (
                general_mem,
                lazy_ptr_mem,
                start_time.elapsed(),
            )
        }
    };
}

#[test]
fn check_ram_consumption() {
    let res = (0..4)
        .map(|_| {
            let rcugc = check! {
                let rcu = RcuGC::new(0);
                thread::scope(|s| {
                    for _ in 0..1_000 {
                        s.spawn(|| {
                            for _ in 0..1_000 {
                                rcu.change(|data| {
                                    *data += 1;
                                });
                            }
                        });
                    }
                });
            };
            let rcu = check!(
                let rcu = Rcu::new(0);
                thread::scope(|s| {
                    for _ in 0..1_000 {
                        s.spawn(|| {
                            for _ in 0..1_000 {
                                rcu.change(|data| {
                                    *data += 1;
                                });
                            }
                        });
                    }
                });
            );
            assert!(rcugc < rcu);
            [
                rcu.0 as f64 / rcugc.0 as f64,
                rcu.1 as f64 - rcugc.1 as f64,
                rcugc.2.div_duration_f64(rcu.2),
            ]
        })
        .fold([const { Vec::new() }; 3], |mut arr, curr| {
            arr.iter_mut().enumerate().for_each(|(i, ar)| {
                ar.push(curr[i]);
            });
            arr
        });

    println!(
        "По окончанию теста:\n{}",
        [
            |arg| format!("\trcu тратит в общем памяти в {:.2} раз меньше", arg),
            |arg| format!("\trcu содержит ленивых указателей на {:.}", arg),
            |arg| format!("\trcugc выполняется в {:.2} медленнее", arg),
        ]
        .iter()
        .zip(res.iter())
        .map(|(f, res)| { f(res.iter().sum::<f64>() / res.len().as_::<f64>()) })
        .collect::<Vec<_>>()
        .join("\n")
    );
}
