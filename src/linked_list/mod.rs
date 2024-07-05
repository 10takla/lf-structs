use crate::rcu::Rcu;
use std::cell::{Cell, UnsafeCell};
use std::collections::HashSet;
use std::fmt::{Debug, Display};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::thread;
use std::time::Instant;
use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicPtr, Ordering::*},
};
use std_reset::traits::of_to::{Of, To};

pub mod new_solution;

#[derive(Debug)]
struct Node<T> {
    data: Rcu<T>,
    next: AtomicPtr<Node<T>>,
}
unsafe impl<T: Send> Sync for Node<T> {}

impl<T: Clone> Node<T> {
    pub fn data(&self) -> T {
        self.data.load()
    }
}

impl<T: Clone> Node<T> {
    pub fn new(data: T) -> Self {
        Self {
            data: Rcu::new(data),
            next: AtomicPtr::new(std::ptr::null_mut()),
        }
    }
}

impl<T: Display + Debug + Clone> Display for Node<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.data())
    }
}

#[derive(Debug)]
pub struct List<T> {
    head: AtomicPtr<Node<T>>,
    foot: AtomicPtr<Node<T>>,
}

unsafe impl<T: Send> Sync for List<T> {}

impl<T: Display + Debug + Clone> Display for List<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.to::<Vec<T>>().iter()).finish()
    }
}

impl<T: Clone> Of<&List<T>> for Vec<T> {
    fn of(s: &List<T>) -> Vec<T> {
        let mut d_l = vec![];
        if let Some(mut u) = unsafe { s.head.load(Relaxed).as_ref() } {
            loop {
                d_l.push(u.data());
                if let Some(node) = unsafe { u.next.load(Relaxed).as_ref() } {
                    u = node;
                } else {
                    break;
                }
            }
        }
        d_l
    }
}

impl<T: Clone + Debug> List<T> {
    pub fn new(data: &[T]) -> Self {
        let Some((head, data)) = data.split_first() else {
            return Self {
                head: AtomicPtr::new(std::ptr::null_mut()),
                foot: AtomicPtr::new(std::ptr::null_mut()),
            };
        };

        let mut node = Node::new(head.clone());
        let mut curr = &mut node.next;

        let Some((foot, data)) = data.split_last() else {
            let ptr = Box::into_raw(Box::new(node));
            return Self {
                head: AtomicPtr::new(ptr),
                foot: AtomicPtr::new(ptr),
            };
        };

        for i in data {
            let node = Node::new(i.clone());
            curr.swap(Box::into_raw(Box::new(node)), Relaxed);
            curr = &mut unsafe { &mut *curr.load(Relaxed) }.next;
        }

        let foot = Node::new(foot.clone());
        let f_ptr: *mut Node<T> = Box::into_raw(Box::new(foot));
        curr.swap(f_ptr, Relaxed);

        Self {
            head: AtomicPtr::new(Box::into_raw(Box::new(node))),
            foot: AtomicPtr::new(f_ptr),
        }
    }
    pub fn push_front(&self, data: T) {
        let new_node = Box::into_raw(Box::new(Node::new(data.clone())));
        let mut head = self.head.load(Acquire);
        loop {
            if !head.is_null() {
                unsafe { (*new_node).next.store(head, Relaxed) };
                match self.head.compare_exchange(head, new_node, Release, Relaxed) {
                    Ok(_) => break,
                    Err(t) => head = t,
                }
            } else {
                if self
                    .head
                    .compare_exchange(std::ptr::null_mut(), new_node, Release, Relaxed)
                    .is_ok()
                {
                    self.foot.store(std::ptr::null_mut(), Release);
                    break;
                }
            }
        }
    }
    pub fn push_back(&self, data: T)
    where
        T: Display,
    {
        let new_node = Box::into_raw(Box::new(Node::new(data.clone())));

        loop {
            if let Some(foot_node) = unsafe { self.foot.load(Acquire).as_ref() } {
                if foot_node
                    .next
                    .compare_exchange(std::ptr::null_mut(), new_node, Release, Relaxed)
                    .is_err()
                {
                    continue;
                };
            } else {
                if self
                    .head
                    .compare_exchange(std::ptr::null_mut(), new_node, Release, Relaxed)
                    .is_err()
                {
                    continue;
                };
            }
            self.foot.store(new_node, Release);
            break;
        }
    }
}

#[test]
fn new_list() {
    let data = &[1, 2, 3, 4, 5];
    let list = &dbg!(List::new(data));
    assert_eq!(list.to::<Vec<usize>>().len(), data.len());
}

#[test]
fn push_front_back() {
    let list: &List<isize> = &List::new(&[]);
    const N: usize = 399;

    let start = Instant::now();
    thread::scope(|s| {
        for i in 1..=N {
            s.spawn(move || {
                list.push_back(i as isize);
            });
        }
        for i in 1..=N {
            s.spawn(move || {
                list.push_front(i as isize * -1);
            });
        }
    });
    dbg!(start.elapsed());

    let mut vec = list.to::<Vec<isize>>();
    assert_eq!(vec.len(), N * 2);
    vec.dedup_by(|a, b| a == b);
    assert_eq!(vec.len(), N * 2);

    assert!(vec[vec.len() / 2] > 0);
    assert!(vec[vec.len() / 2 - 1] < 0);
}

#[test]
fn push_back() {
    let list = &List::new(&[]);
    const N: usize = 10_000;

    let start = Instant::now();
    thread::scope(|s| {
        for i in 0..N {
            s.spawn(move || {
                list.push_back(i);
            });
        }
    });
    dbg!(start.elapsed());

    let mut vec = list.to::<Vec<usize>>();
    assert_eq!(vec.len(), N);
    vec.dedup_by(|a, b| a == b);
    assert_eq!(vec.len(), N);

    let mut queue = VecDeque::new();
    std::hint::black_box(&queue);
    let start = Instant::now();
    for i in 0..N {
        std::hint::black_box(&i);
        queue.push_front(i);
    }
    dbg!(start.elapsed());
}

#[test]
fn push_front() {
    let list = &List::new(&[]);
    const N: usize = 40_000;

    let start = Instant::now();
    thread::scope(|s| {
        for i in 0..N {
            s.spawn(move || {
                list.push_front(i);
            });
        }
    });
    dbg!(start.elapsed());

    let mut vec = list.to::<Vec<usize>>();
    assert_eq!(vec.len(), N);
    vec.dedup_by(|a, b| a == b);
    assert_eq!(vec.len(), N);

    let mut queue = VecDeque::new();
    std::hint::black_box(&queue);
    let start = Instant::now();
    for i in 0..N {
        std::hint::black_box(&i);
        queue.push_front(i);
    }
    dbg!(start.elapsed());
}
