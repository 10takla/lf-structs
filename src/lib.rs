#![feature(test)]
extern crate test;
extern crate atomic_wait;

pub mod semaphore;

pub use semaphore::*;
