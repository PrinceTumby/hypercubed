use core::convert::Infallible;

pub use core::sync::atomic;

pub use alloc::sync::Arc;

#[repr(transparent)]
pub struct Mutex<T: ?Sized>(spin::Mutex<T>);

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self(spin::Mutex::new(value))
    }
}

impl<T: ?Sized> Mutex<T> {
    pub fn lock<'a>(&'a self) -> Result<spin::MutexGuard<'a, T>, Infallible> {
        Ok(self.0.lock())
    }
}

pub mod mpsc {
    use super::super::{Arc, VecDeque};
    use super::{Infallible, Mutex};

    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        (Sender(queue.clone()), Receiver(queue))
    }

    pub struct Sender<T>(Arc<Mutex<VecDeque<T>>>);

    impl<T> Sender<T> {
        pub fn send(&self, t: T) -> Result<(), Infallible> {
            self.0.lock().unwrap().push_back(t);
            Ok(())
        }
    }

    impl<T> Clone for Sender<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }

        fn clone_from(&mut self, source: &Self) {
            self.0.clone_from(&source.0)
        }
    }

    pub struct Receiver<T>(Arc<Mutex<VecDeque<T>>>);

    impl<T> Receiver<T> {
        pub fn try_recv(&self) -> Result<T, TryRecvError> {
            self.0
                .lock()
                .unwrap()
                .pop_front()
                .ok_or(TryRecvError::Empty)
        }
    }

    #[non_exhaustive]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum TryRecvError {
        Empty,
    }

    impl core::fmt::Display for TryRecvError {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            match self {
                Self::Empty => "receiving on an empty channel".fmt(f),
            }
        }
    }

    impl core::error::Error for TryRecvError {}
}
