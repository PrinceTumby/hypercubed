//! As of 2025-08-08, this is needed the link errors complaining about undefined references to
//! `compiler_builtins` functions.
//! Setting `no-f16-f128` in the `rust-toolchain.toml` has had no effect so far.

// `fn __extendhfsf2(a: f16) -> f32`
#[unsafe(no_mangle)]
pub extern "C" fn __extendhfsf2(_a: u16) -> u32 {
    unimplemented!();
}

// `fn __extendhfsf2(a: f32) -> f16`
#[unsafe(no_mangle)]
pub extern "C" fn __truncsfhf2(_a: u32) -> u16 {
    unimplemented!();
}
