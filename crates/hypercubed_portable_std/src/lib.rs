#![cfg_attr(not(feature = "std"), no_std)]

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        mod with_std;
        pub use with_std::*;
    } else {
        extern crate alloc;
        mod without_std;
        pub use without_std::*;
    }
}
