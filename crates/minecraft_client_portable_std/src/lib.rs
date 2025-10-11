#![allow(unexpected_cfgs)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(
    all(target_arch = "mips", target_vendor = "sony", target_os = "ps2"),
    feature(asm_experimental_arch)
)]

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
