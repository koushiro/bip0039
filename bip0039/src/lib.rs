//! Another Rust implementation of [BIP-0039](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki) standard.
//!
//! ## Usage
//!
//! ```rust
//! use bip0039::{Count, Mnemonic, ChineseSimplified};
//!
//! /// Generates an English mnemonic with 12 words randomly
//! let mnemonic = <Mnemonic>::generate(Count::Words12);
//!
//! /// Gets the phrase
//! let phrase = mnemonic.phrase();
//! println!("phrase: {}", phrase);
//!
//! /// Generates the HD wallet seed from the mnemonic and the passphrase.
//! let seed = mnemonic.to_seed("");
//! println!("seed: {}", hex::encode(&seed[..]));
//!
//! /// Generates a Simplified Chinese mnemonic with 12 words randomly
//! let mnemonic = <Mnemonic<ChineseSimplified>>::generate(Count::Words12);
//! println!("phrase: {}", mnemonic.phrase());
//! ```
//!

#![deny(unused_imports)]
#![deny(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

mod error;
/// Supported languages for BIP-0039.
pub mod language;
mod mnemonic;

pub use self::{
    error::Error,
    language::{English, Lang},
    mnemonic::{Count, Mnemonic},
};

#[cfg(feature = "chinese-simplified")]
pub use self::language::ChineseSimplified;
#[cfg(feature = "chinese-traditional")]
pub use self::language::ChineseTraditional;
#[cfg(feature = "czech")]
pub use self::language::Czech;
#[cfg(feature = "french")]
pub use self::language::French;
#[cfg(feature = "italian")]
pub use self::language::Italian;
#[cfg(feature = "japanese")]
pub use self::language::Japanese;
#[cfg(feature = "korean")]
pub use self::language::Korean;
#[cfg(feature = "portuguese")]
pub use self::language::Portuguese;
#[cfg(feature = "spanish")]
pub use self::language::Spanish;
