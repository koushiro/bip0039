//! Another Rust implementation of [BIP-0039](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki) standard.

#![deny(unused_imports)]
#![deny(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

mod error;
mod language;
mod mnemonic;

pub use self::error::Error;
pub use self::language::Language;
pub use self::mnemonic::{Count, Mnemonic};
