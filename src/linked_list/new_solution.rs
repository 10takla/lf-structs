use crate::rcu::Rcu;

struct List<T> {
    rcu: Rcu<T>,
}

impl<T: Clone> List<T> {
    pub fn new(data: T) -> Self {
        Self {
            rcu: Rcu::new(data),
        }
    }
}
