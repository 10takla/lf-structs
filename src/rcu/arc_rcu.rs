use crate::check;
use super::*;
use std_reset::{prelude::Deref, traits::as_prim::AsPrim};

#[derive(Deref)]
pub struct ArcRcu<T> {
    #[deref]
    rcu: Rcu<T>,
}

unsafe impl<T> Sync for ArcRcu<T> where T: Send {}
unsafe impl<T> Send for ArcRcu<T> where T: Send {}

impl<T: Clone> ArcRcu<T> {
    pub fn new(data: T) -> Self {
        Self {
            rcu: Rcu::new(data),
        }
    }
    pub fn load(&self) -> T {
        self.rcu.load()
    }
    pub fn change(&self, f: impl Fn(&mut T)) {
        let mut load_data = self.ptr.load(Acquire);

        loop {
            let mut changed_data = unsafe { &mut *(load_data.clone()) }.clone();
            f(&mut changed_data);
            let new_ptr = Box::into_raw(Box::new(changed_data));

            match self
                .ptr
                .compare_exchange(load_data, new_ptr, AcqRel, Relaxed)
            {
                Ok(_) => {
                    // Освобождаем текущий Arc
                    // drop(load_data);
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
    }
}

#[test]
fn check_ram_consumption() {
    let res = (0..4)
        .map(|_| {
            let arcrcu = check! {
                let rcu = ArcRcu::new(0);
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
            assert!(arcrcu < rcu);
            [
                rcu.0 as f64 / arcrcu.0 as f64,
                rcu.1 as f64 - arcrcu.1 as f64,
                arcrcu.2.div_duration_f64(rcu.2),
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
