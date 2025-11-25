#![cfg_attr(not(feature = "full_std"), no_main)]
#![cfg_attr(not(feature = "mini_std"), no_std)]

#[cfg(feature = "full_std")]
pub fn main() -> anyhow::Result<()> {
    hypercubed::platform::main()
}
