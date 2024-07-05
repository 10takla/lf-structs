use lf_structs::{linked_list::List, rcu::Rcu};
use std::{
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};
use std_reset::prelude::Display;

#[derive(Debug, Clone, Display)]
struct User {
    id: usize,
    name: String,
    password: String,
}
const COUNT: usize = 100_000;
fn main() {
    let list = &Rcu::new(
        (1..=COUNT)
            .map(|i| {
                Rcu::new(User {
                    id: i,
                    name: format!("name {i}"),
                    password: format!("password {i}"),
                })
            })
            .collect::<Vec<_>>(),
    );

    let start = Instant::now();
    thread::scope(|s| {
        s.spawn(|| {
            list.change(|users| {
                thread::scope(|s| {
                    for (i, user) in users.into_iter().enumerate() {
                        if i % 2 == 0 {
                            s.spawn(move || {
                                user.change(|user| {
                                    user.name = "Ken".to_string();
                                });
                            });
                        }
                    }
                });
            });
        });
        s.spawn(move || {
            list.change(|users| {
                thread::scope(|s| {
                    for (i, user) in users.into_iter().enumerate() {
                        if i % 2 == 1 {
                            s.spawn(move || {
                                user.change(|user| {
                                    user.name = "David".to_string();
                                });
                            });
                        }
                    }
                });
            });
        });
    });
    dbg!(start.elapsed());

    let mut users = (1..=COUNT)
        .map(|i| User {
            id: i,
            name: format!("name {i}"),
            password: format!("password {i}"),
        })
        .collect::<Vec<_>>();

    let start = Instant::now();
    for (i, user) in users.iter_mut().enumerate() {
        if i % 2 == 1 {
            user.name = "Ken".to_string();
        }
    }
    for (i, user) in users.iter_mut().enumerate() {
        if i % 2 == 1 {
            user.name = "David".to_string();
        }
    }
    dbg!(start.elapsed());

    let users = Mutex::new(
        (1..=COUNT)
            .map(|i| User {
                id: i,
                name: format!("name {i}"),
                password: format!("password {i}"),
            })
            .collect::<Vec<_>>(),
    );

    let start = Instant::now();
    thread::scope(|s| {
        s.spawn(|| {
            let mut users = users.lock().unwrap();
            thread::scope(|s| {
                for (i, user) in users.iter_mut().enumerate() {
                    if i % 2 == 0 {
                        s.spawn(move || {
                            user.name = "Ken".to_string();
                        });
                    }
                }
            });
        });
        s.spawn(|| {
            let mut users = users.lock().unwrap();
            thread::scope(|s| {
                for (i, user) in users.iter_mut().enumerate() {
                    if i % 2 == 1 {
                        s.spawn(move || {
                            user.name = "David".to_string();
                            (*user).clone()
                        });
                    }
                }
            });
            (*users).clone()
        });
    });
    dbg!(start.elapsed());
}
