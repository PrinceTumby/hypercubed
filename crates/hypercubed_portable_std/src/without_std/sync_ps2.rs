use core::cell::UnsafeCell;
use core::convert::Infallible;
use core::marker::PhantomData;
use core::ptr::NonNull;

use super::Box;
use atomic::{AtomicBool, AtomicUsize, Ordering};

pub struct Arc<T: ?Sized> {
    ptr: NonNull<ArcInner<T>>,
    phantom: PhantomData<ArcInner<T>>,
}

#[repr(C)]
struct ArcInner<T: ?Sized> {
    strong: AtomicUsize,
    data: T,
}

unsafe impl<T: ?Sized + Sync + Send> Sync for Arc<T> {}
unsafe impl<T: ?Sized + Sync + Send> Send for Arc<T> {}

impl<T> Arc<T> {
    pub fn new(data: T) -> Self {
        let x = Box::new(ArcInner {
            strong: AtomicUsize::new(1),
            data,
        });
        unsafe { Self::from_inner(NonNull::new_unchecked(Box::into_raw(x))) }
    }
}

impl<T: Clone> Arc<T> {
    pub fn make_mut<'a>(this: &'a mut Self) -> &'a mut T {
        if this.inner().strong.load(Ordering::Acquire) > 1 {
            // If there's another reference, then clone the data.
            let x = Box::new(ArcInner {
                strong: AtomicUsize::new(1),
                data: this.inner().data.clone(),
            });
            *this = unsafe { Self::from_inner(NonNull::new_unchecked(Box::into_raw(x))) };
        }
        // We either already were, or are now, the only reference to the data, so this should be
        // safe.
        unsafe { &mut (*this.ptr.as_ptr()).data }
    }
}

impl<T: ?Sized> Arc<T> {
    unsafe fn from_inner(ptr: NonNull<ArcInner<T>>) -> Self {
        Self {
            ptr,
            phantom: PhantomData,
        }
    }

    #[inline]
    fn inner(&self) -> &ArcInner<T> {
        unsafe { self.ptr.as_ref() }
    }

    #[inline(never)]
    unsafe fn drop_slow(&mut self) {
        unsafe {
            drop(Box::from_raw(self.ptr.as_ptr()));
        }
    }
}

impl<T: ?Sized> Clone for Arc<T> {
    #[inline]
    fn clone(&self) -> Self {
        self.inner().strong.fetch_add(1, Ordering::Relaxed);
        unsafe { Self::from_inner(self.ptr) }
    }
}

impl<T: ?Sized> Drop for Arc<T> {
    #[inline]
    fn drop(&mut self) {
        if self.inner().strong.fetch_sub(1, Ordering::Release) != 1 {
            return;
        }
        // If this is the last reference, drop the data.
        unsafe {
            self.drop_slow();
        }
    }
}

impl<T: ?Sized> core::ops::Deref for Arc<T> {
    type Target = T;

    fn deref<'a>(&'a self) -> &'a T {
        &self.inner().data
    }
}

impl<T: ?Sized> core::borrow::Borrow<T> for Arc<T> {
    fn borrow<'a>(&'a self) -> &'a T {
        &**self
    }
}

impl<T: ?Sized> AsRef<T> for Arc<T> {
    fn as_ref<'a>(&'a self) -> &'a T {
        &**self
    }
}

pub struct Mutex<T: ?Sized> {
    lock: AtomicBool,
    data: UnsafeCell<T>,
}

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(value),
        }
    }
}

impl<T: ?Sized> Mutex<T> {
    pub fn lock<'a>(&'a self) -> Result<MutexGuard<'a, T>, Infallible> {
        loop {
            if let Some(guard) = self.try_lock() {
                return Ok(guard);
            }
            // Workaround for R5900 short loop bug.
            unsafe {
                core::arch::asm!("nop", "nop", "nop", "nop", "nop", "nop");
            }
        }
    }

    fn try_lock<'a>(&'a self) -> Option<MutexGuard<'a, T>> {
        let already_locked = self.lock.swap(true, Ordering::Acquire);
        if already_locked {
            None
        } else {
            Some(MutexGuard {
                lock: &self.lock,
                data: self.data.get(),
            })
        }
    }
}

pub struct MutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a AtomicBool,
    data: *mut T,
}

impl<'a, T: ?Sized + core::fmt::Debug> core::fmt::Debug for MutexGuard<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized + core::fmt::Display> core::fmt::Display for MutexGuard<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> core::ops::Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.data }
    }
}

impl<'a, T: ?Sized> core::ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data }
    }
}

impl<'a, T: ?Sized> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.store(false, atomic::Ordering::Release);
    }
}

pub mod atomic {
    use core::cell::UnsafeCell;
    pub use core::sync::atomic::Ordering;

    // If we're compiling for PS2, we're linking to `asm_helpers.s` in the main
    // `src/platform/ps2` directory.
    unsafe extern "C" {
        unsafe fn disable_interrupts() -> bool;

        unsafe fn enable_interrupts() -> bool;
    }

    fn with_interrupts_disabled<T>(f: impl FnOnce() -> T) -> T {
        unsafe {
            let interrupts_previously_enabled = disable_interrupts();
            let val = f();
            if interrupts_previously_enabled {
                enable_interrupts();
            }
            val
        }
    }

    macro_rules! create_atomic {
        ($name:ident, $inner_type:ty) => {
            #[repr(transparent)]
            pub struct $name(UnsafeCell<$inner_type>);

            impl $name {
                pub const fn new(v: $inner_type) -> Self {
                    Self(UnsafeCell::new(v))
                }

                pub fn load(&self, order: Ordering) -> $inner_type {
                    let val = unsafe { *self.0.get() };
                    match order {
                        Ordering::Relaxed | Ordering::Release => {}
                        _ => core::sync::atomic::compiler_fence(order),
                    }
                    val
                }

                pub fn store(&self, val: $inner_type, order: Ordering) {
                    unsafe {
                        *self.0.get() = val;
                    }
                    match order {
                        Ordering::Relaxed | Ordering::Acquire => {}
                        _ => core::sync::atomic::compiler_fence(order),
                    }
                }

                pub fn swap(&self, val: $inner_type, order: Ordering) -> $inner_type {
                    with_interrupts_disabled(|| {
                        let current_val = self.load(order);
                        self.store(val, order);
                        current_val
                    })
                }
            }

            impl Default for $name {
                #[inline]
                fn default() -> Self {
                    Self::new(Default::default())
                }
            }

            impl From<$inner_type> for $name {
                #[inline]
                fn from(v: $inner_type) -> Self {
                    Self::new(v)
                }
            }

            impl core::fmt::Debug for $name {
                fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                    core::fmt::Debug::fmt(&self.load(Ordering::Relaxed), f)
                }
            }

            unsafe impl Sync for $name {}
        };
    }

    macro_rules! create_atomic_int {
        ($name:ident, $inner_type:ty) => {
            create_atomic!($name, $inner_type);

            impl $name {
                pub fn fetch_add(&self, val: $inner_type, order: Ordering) -> $inner_type {
                    with_interrupts_disabled(|| {
                        let current_val = self.load(order);
                        self.store(current_val.wrapping_add(val), order);
                        current_val
                    })
                }

                pub fn fetch_sub(&self, val: $inner_type, order: Ordering) -> $inner_type {
                    with_interrupts_disabled(|| {
                        let current_val = self.load(order);
                        self.store(current_val.wrapping_sub(val), order);
                        current_val
                    })
                }
            }
        };
    }

    create_atomic!(AtomicBool, bool);
    create_atomic_int!(AtomicU8, u8);
    create_atomic_int!(AtomicI8, i8);
    create_atomic_int!(AtomicU16, u16);
    create_atomic_int!(AtomicI16, i16);
    create_atomic_int!(AtomicU32, u32);
    create_atomic_int!(AtomicI32, i32);
    create_atomic_int!(AtomicUsize, usize);
    create_atomic_int!(AtomicIsize, isize);
}

pub mod mpsc {
    use super::super::VecDeque;
    use super::{Arc, Infallible, Mutex};

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
