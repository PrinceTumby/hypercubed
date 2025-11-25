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
#![cfg_attr(not(feature = "mini_std"), no_std)]
#![cfg_attr(feature = "platform_ps2", feature(asm_experimental_arch))]

#[cfg(not(any(feature = "mini_std", test)))]
#[macro_use]
extern crate alloc;

// extern crate portable_std as std;
// TODO: Try this (^), to see if we can simplify imports in crates.

pub mod portable_prelude {
    pub use portable_std::prelude::*;

    cfg_if::cfg_if! {
        if #[cfg(not(feature = "full_std"))] {
            #[allow(unused)]
            pub(crate) use crate::platform::{dbg, println, eprintln};
            pub use nalgebra::{ComplexField, RealField};
        } else {
            pub use std::{dbg, println, eprintln};
        }
    }
}

pub mod basic_types;
pub mod client;
pub mod physics;
pub mod platform;
pub mod protocol;
pub mod world;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;
