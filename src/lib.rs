#![warn(clippy::all)]
#![deny(clippy::correctness)]
#![deny(
    clippy::std_instead_of_core,
    reason = "we want to be portable to platforms with `no_std`"
)]
#![deny(
    clippy::std_instead_of_alloc,
    reason = "the types in `crate::prelude` can be used for portability"
)]
#![deny(clippy::alloc_instead_of_core)]
#![cfg_attr(all(not(test), not(feature = "std")), no_std)]
#![cfg_attr(feature = "platform_ps2", feature(asm_experimental_arch))]

#[cfg(not(feature = "std"))]
#[macro_use]
extern crate alloc;

pub mod portable_prelude {
    pub use portable_std::prelude::*;
    
    cfg_if::cfg_if! {
        if #[cfg(not(feature = "std"))] {
            #[allow(unused)]
            pub(crate) use crate::platform::{dbg, println, eprintln};
            pub use nalgebra::{ComplexField, RealField};
        }
    }
}

pub mod basic_types;
pub mod client;
pub mod physics;
pub mod platform;
pub mod protocol;
pub mod world;

#[cfg(all(
    feature = "std",
    any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ),
))]
#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;
