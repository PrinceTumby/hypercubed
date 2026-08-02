#![cfg_attr(not(feature = "std"), no_std)]

cfg_select! {
    feature = "std" => {
        mod with_std;
        pub use with_std::*;
    }
    _ => {
        extern crate alloc;
        mod without_std;
        pub use without_std::*;
    }
}
