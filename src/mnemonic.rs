#[cfg(not(feature = "std"))]
use alloc::{
    borrow::Cow,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::{convert, fmt, ops::Range, str};
#[cfg(feature = "std")]
use std::borrow::Cow;

use hmac::Hmac;
use sha2::{Digest, Sha256, Sha512};
use zeroize::Zeroize;

use crate::error::Error;
use crate::language::Language;

const BITS_PER_WORD: usize = 11;
const BITS_PER_BYTE: usize = 8;
const ENTROPY_OFFSET: usize = 8;

/// Determines the words count that will be present in a [`Mnemonic`] phrase.
#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum Count {
    /// 12 words, entropy length: 128 bits, the checksum length: 4 bits.
    Words12 = (128 << ENTROPY_OFFSET) | 4,
    /// 15 words, entropy length: 160 bits, the checksum length: 5 bits.
    Words15 = (160 << ENTROPY_OFFSET) | 5,
    /// 18 words, entropy length: 192 bits, the checksum length: 6 bits.
    Words18 = (192 << ENTROPY_OFFSET) | 6,
    /// 21 words, entropy length: 224 bits, the checksum length: 7 bits.
    Words21 = (224 << ENTROPY_OFFSET) | 7,
    /// 24 words, entropy length: 256 bits, the checksum length: 8 bits.
    Words24 = (256 << ENTROPY_OFFSET) | 8,
}

impl Default for Count {
    fn default() -> Self {
        Self::Words12
    }
}

impl fmt::Display for Count {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} words (entropy {} bits + checksum {} bits)",
            self.word_count(),
            self.entropy_bits(),
            self.checksum_bits()
        )
    }
}

impl From<Count> for usize {
    fn from(count: Count) -> Self {
        match count {
            Count::Words12 => 12,
            Count::Words15 => 15,
            Count::Words18 => 18,
            Count::Words21 => 21,
            Count::Words24 => 24,
        }
    }
}

impl convert::TryFrom<usize> for Count {
    type Error = Error;

    fn try_from(count: usize) -> Result<Self, Self::Error> {
        Self::from_word_count(count)
    }
}

impl Count {
    /// Creates a [`Count`] for a mnemonic phrase with the given word count.
    // TODO: #![feature(const_if_match)] has been stabilized in 1.46+.
    fn from_word_count(count: usize) -> Result<Self, Error> {
        Ok(match count {
            12 => Self::Words12,
            15 => Self::Words15,
            18 => Self::Words18,
            21 => Self::Words21,
            24 => Self::Words24,
            others => return Err(Error::BadWordCount(others)),
        })
    }

    /// Creates a [`Count`] for a mnemonic phrase with the given entropy bits size.
    // TODO: #![feature(const_if_match)] has been stabilized in 1.46+.
    fn from_key_size(size: usize) -> Result<Self, Error> {
        Ok(match size {
            128 => Self::Words12,
            160 => Self::Words15,
            192 => Self::Words18,
            224 => Self::Words21,
            256 => Self::Words24,
            others => return Err(Error::BadEntropyBitCount(others)),
        })
    }

    /// Creates a [`Count`] for an existing mnemonic phrase.
    fn from_phrase<P: AsRef<str>>(phrase: P) -> Result<Self, Error> {
        let word_count = phrase.as_ref().split_whitespace().count();
        Self::from_word_count(word_count)
    }

    /// Returns the number of words.
    pub const fn word_count(&self) -> usize {
        self.total_bits() / BITS_PER_WORD
    }

    /// Returns the number of entropy+checksum bits.
    pub const fn total_bits(&self) -> usize {
        self.entropy_bits() + self.checksum_bits()
    }

    /// Returns the number of entropy bits.
    pub const fn entropy_bits(&self) -> usize {
        (*self as usize) >> ENTROPY_OFFSET
    }

    /// Returns the number of checksum bits.
    pub const fn checksum_bits(&self) -> usize {
        (*self as usize) as u8 as usize
    }

    const fn total(&self) -> Range<usize> {
        0..self.total_bits()
    }

    const fn entropy(&self) -> Range<usize> {
        0..self.entropy_bits()
    }

    const fn checksum(&self) -> Range<usize> {
        self.entropy_bits()..self.total_bits()
    }
}

/// A mnemonic representation.
///
/// First, an initial entropy of ENT bits is generated.
/// A checksum is generated by taking the first `ENT/32` bits of its SHA256 hash.
/// This checksum is appended to the end of the initial entropy.
///
/// Next, these concatenated bits are split into groups of `11` bits,
/// each encoding a number from 0-2047, serving as an index into a wordlist.
///
/// Finally, we convert these numbers into words and use the joined words as a mnemonic sentence.
///
/// - **ENT**: the initial entropy length
/// - **CS**: the checksum length
/// - **MS**: the length of the generated mnemonic sentence in words
///
/// **CS** = **ENT** / 32
///
/// **MS** = (**ENT** + **CS**) / 11
///
/// |  ENT  |  CS  | ENT+CS |  MS  |
/// | :---: | :--: | :----: | :--: |
/// |  128  |  4   |  132   |  12  |
/// |  160  |  5   |  165   |  15  |
/// |  192  |  6   |  198   |  18  |
/// |  224  |  7   |  231   |  21  |
/// |  256  |  8   |  264   |  24  |
///
/// For example, a 12 word mnemonic phrase is essentially a friendly representation of
/// a 128-bit key, while a 24 word mnemonic phrase is essentially a 256-bit key.
///
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Mnemonic {
    lang: Language,
    phrase: String,
    entropy: Vec<u8>,
}

impl fmt::Debug for Mnemonic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.phrase())
    }
}

impl fmt::Display for Mnemonic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.phrase())
    }
}

impl str::FromStr for Mnemonic {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_phrase(s)
    }
}

impl AsRef<str> for Mnemonic {
    fn as_ref(&self) -> &str {
        self.phrase()
    }
}

impl Zeroize for Mnemonic {
    fn zeroize(&mut self) {
        self.phrase.zeroize();
        self.entropy.zeroize();
    }
}

impl Drop for Mnemonic {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl Mnemonic {
    /// Generate a new English [`Mnemonic`] in the specified word count.
    ///
    /// # Example
    ///
    /// ```
    /// use bip0039::{Count, Mnemonic};
    ///
    /// let mnemonic = Mnemonic::generate(Count::Words12);
    /// let phrase = mnemonic.phrase();
    /// ```
    #[cfg(feature = "rand")]
    pub fn generate(word_count: Count) -> Self {
        Self::generate_in(Language::English, word_count)
    }

    /// Generate a new [`Mnemonic`] in the specified language and word count.
    ///
    /// # Example
    ///
    /// ```
    /// use bip0039::{Count, Language, Mnemonic};
    ///
    /// let mnemonic = Mnemonic::generate_in(Language::SimplifiedChinese, Count::Words24);
    /// let phrase = mnemonic.phrase();
    /// ```
    #[cfg(feature = "rand")]
    pub fn generate_in(lang: Language, word_count: Count) -> Self {
        use rand::RngCore;
        const MAX_ENTROPY_BITS: usize = Count::Words24.entropy_bits();

        let mut rng = rand::thread_rng();
        let mut entropy = [0u8; MAX_ENTROPY_BITS / BITS_PER_BYTE];
        rng.fill_bytes(&mut entropy);

        let entropy_bytes = word_count.entropy_bits() / BITS_PER_BYTE;
        Self::from_entropy_in(lang, &entropy[..entropy_bytes])
            .expect("valid entropy length won't fail to generate the mnemonic")
    }

    /// Creates a new English [`Mnemonic`] from the given entropy.
    ///
    /// # Example
    ///
    /// ```
    /// use bip0039::Mnemonic;
    ///
    /// let entropy = vec![0x1a, 0x48, 0x6a, 0x5f, 0xbe, 0x53, 0x63, 0x99, 0x84, 0xcb, 0x64, 0xb0, 0x70, 0x75, 0x5f, 0x7b];
    /// let mnemonic = Mnemonic::from_entropy(entropy).unwrap();
    /// assert_eq!(mnemonic.phrase(), "bottom drive obey lake curtain smoke basket hold race lonely fit walk");
    /// ```
    pub fn from_entropy<E: Into<Vec<u8>>>(entropy: E) -> Result<Self, Error> {
        Self::from_entropy_in(Language::English, entropy)
    }

    /// Creates a new [`Mnemonic`] in the specified language from the given entropy.
    ///
    /// # Example
    ///
    /// ```
    /// use bip0039::{Language, Mnemonic};
    ///
    /// let entropy = vec![0x1a, 0x48, 0x6a, 0x5f, 0xbe, 0x53, 0x63, 0x99, 0x84, 0xcb, 0x64, 0xb0, 0x70, 0x75, 0x5f, 0x7b];
    /// let mnemonic = Mnemonic::from_entropy_in(Language::English, entropy).unwrap();
    /// assert_eq!(mnemonic.phrase(), "bottom drive obey lake curtain smoke basket hold race lonely fit walk");
    /// ```
    pub fn from_entropy_in<E: Into<Vec<u8>>>(lang: Language, entropy: E) -> Result<Self, Error> {
        const MAX_TOTAL_BITS: usize = Count::Words24.total_bits();

        let entropy = entropy.into();
        let word_count = Count::from_key_size(entropy.len() * BITS_PER_BYTE)?;

        // An initial entropy of ENT bits is given.
        let mut bits = [false; MAX_TOTAL_BITS];
        for (index, bit) in bits[word_count.entropy()].iter_mut().enumerate() {
            *bit = left_index_bit(entropy[index / BITS_PER_BYTE], index % BITS_PER_BYTE);
        }
        // A checksum is generated by taking the first `ENT/32` bits of its SHA256 hash.
        // and this checksum is appended to the end of the initial entropy.
        let checksum_byte = Sha256::digest(&entropy)[0];
        for (index, bit) in bits[word_count.checksum()].iter_mut().enumerate() {
            *bit = left_index_bit(checksum_byte, index);
        }

        let mut words = Vec::with_capacity(word_count.word_count());
        for chunk in bits[word_count.total()].chunks(BITS_PER_WORD) {
            let index = bits_to_uint(chunk, BITS_PER_WORD);
            words.push(lang.word_of(index));
        }
        let phrase = words.join(" ");

        Ok(Self {
            lang,
            phrase,
            entropy,
        })
    }

    /// Creates a [`Mnemonic`] from an existing mnemonic phrase.
    ///
    /// # Example
    ///
    /// ```
    /// use bip0039::Mnemonic;
    ///
    /// let phrase = "bottom drive obey lake curtain smoke basket hold race lonely fit walk";
    /// let mnemonic = Mnemonic::from_phrase(phrase).unwrap();
    /// assert_eq!(mnemonic.phrase(), phrase);
    /// ```
    pub fn from_phrase<'a, P: Into<Cow<'a, str>>>(phrase: P) -> Result<Self, Error> {
        Self::from_phrase_in(Language::English, phrase)
    }

    /// Creates a [`Mnemonic`] from an existing mnemonic phrase in the given language.
    ///
    /// # Example
    ///
    /// ```
    /// use bip0039::{Error, Mnemonic, Language};
    ///
    /// let phrase = "bottom drive obey lake curtain smoke basket hold race lonely fit walk";
    /// let mnemonic = Mnemonic::from_phrase_in(Language::English, phrase).unwrap();
    /// assert_eq!(mnemonic.phrase(), phrase);
    ///
    /// let phrase = "bottom drive obey lake curtain smoke basket hold race lonely fit shit";
    /// let mnemonic = Mnemonic::from_phrase_in(Language::English, phrase);
    /// assert_eq!(mnemonic.unwrap_err(), Error::UnknownWord("shit".into()));
    /// ```
    pub fn from_phrase_in<'a, P: Into<Cow<'a, str>>>(
        lang: Language,
        phrase: P,
    ) -> Result<Self, Error> {
        let phrase = phrase.into();
        let entropy = Self::phrase_to_entropy(lang, phrase.as_ref())?;
        Ok(Mnemonic {
            lang,
            phrase: phrase.into_owned(),
            entropy,
        })
    }

    /// Validates the word count and checksum of an English mnemonic phrase.
    ///
    /// # Example
    ///
    /// ```
    /// use bip0039::Mnemonic;
    ///
    /// let result = Mnemonic::validate("bottom drive obey lake curtain smoke basket hold race lonely fit walk");
    /// assert!(result.is_ok());
    /// ```
    pub fn validate<'a, P: Into<Cow<'a, str>>>(phrase: P) -> Result<(), Error> {
        Self::validate_in(Language::English, phrase)
    }

    /// Validates the word count and checksum of a mnemonic phrase in the given language.
    ///
    /// # Example
    ///
    /// ```
    /// use bip0039::{Error, Language, Mnemonic};
    /// use unicode_normalization::UnicodeNormalization;
    ///
    /// let phrase = "bottom drive obey lake curtain smoke basket hold race lonely fit walk";
    /// let result = Mnemonic::validate_in(Language::English, phrase);
    /// assert!(result.is_ok());
    /// let phrase = "bottom drive obey lake curtain smoke basket hold race lonely fit shit";
    /// let result = Mnemonic::validate_in(Language::English, phrase);
    /// assert_eq!(result.unwrap_err(), Error::UnknownWord("shit".into()));
    ///
    /// let phrase = "そつう　れきだい　ほんやく　わかす　りくつ　ばいか　ろせん　やちん　そつう　れきだい　ほんやく　わかめ";
    /// let result = Mnemonic::validate_in(Language::Japanese, phrase);
    /// assert!(result.is_ok());
    /// let phrase = "そつう　れきだい　ほんやく　わかす　りくつ　ばいか　ろせん　やちん　そつう　れきだい　ほんやく　ばか";
    /// let result = Mnemonic::validate_in(Language::Japanese, phrase);
    /// assert_eq!(result.unwrap_err(), Error::UnknownWord("ばか".nfkd().to_string()));
    /// ```
    pub fn validate_in<'a, P: Into<Cow<'a, str>>>(lang: Language, phrase: P) -> Result<(), Error> {
        let _entropy = Self::phrase_to_entropy(lang, phrase)?;
        Ok(())
    }

    fn phrase_to_entropy<'a, P: Into<Cow<'a, str>>>(
        lang: Language,
        phrase: P,
    ) -> Result<Vec<u8>, Error> {
        let mut phrase = phrase.into();
        normalize_utf8(&mut phrase);
        let word_count = Count::from_phrase(phrase.as_ref())?;

        let mut bits = vec![false; word_count.total_bits()];
        for (i, word) in phrase.split_whitespace().enumerate() {
            if let Some(index) = lang.index_of(word) {
                index_to_bits(index, &mut bits[i * BITS_PER_WORD..], BITS_PER_WORD);
            } else {
                return Err(Error::UnknownWord(word.to_string()));
            }
        }

        let mut entropy = vec![0u8; word_count.entropy_bits() / BITS_PER_BYTE];
        entropy.iter_mut().enumerate().for_each(|(i, byte)| {
            *byte = bits_to_uint(
                &bits[i * BITS_PER_BYTE..(i + 1) * BITS_PER_BYTE],
                BITS_PER_BYTE,
            ) as u8;
        });

        // verify the checksum
        let checksum_bits = &bits[word_count.checksum()];
        let actual_checksum = bits_to_uint(checksum_bits, word_count.checksum_bits()) as u8;

        let checksum_byte = Sha256::digest(&entropy)[0];
        let expected_checksum = checksum(checksum_byte, word_count.checksum_bits());

        if actual_checksum != expected_checksum {
            return Err(Error::InvalidChecksum);
        }

        Ok(entropy)
    }

    /// Generates the seed from the [`Mnemonic`] and the passphrase.
    ///
    /// If a passphrase is not present, an empty string `""` is used instead.
    ///
    /// # Example
    ///
    /// ```
    /// use bip0039::Mnemonic;
    ///
    /// let phrase = "bottom drive obey lake curtain smoke basket hold race lonely fit walk";
    /// let mnemonic = Mnemonic::from_phrase(phrase).unwrap();
    /// assert_eq!(
    ///     mnemonic.to_seed("").to_vec(),
    ///     hex::decode("02d5cd1db85b4d1397d78978062a1160e76e94cc5aaad3089644846865bb18fc68ddf383059d3fe82902a203d60790a8c8ab488de5013d10a8a8bded8d9174b9").unwrap()
    /// );
    /// ```
    pub fn to_seed<P: AsRef<str>>(&self, passphrase: P) -> [u8; 64] {
        // use the PBKDF2 function with a mnemonic sentence (in UTF-8 NFKD) used as the password
        // and the string "mnemonic" + passphrase (again in UTF-8 NFKD) used as the salt.
        // The iteration count is set to 2048 and HMAC-SHA512 is used as the pseudo-random function.
        // The length of the derived key is 512 bits (= 64 bytes).
        const PBKDF2_ROUNDS: u32 = 2048;
        const PBKDF2_BYTES: usize = 64;

        // the phrase has been normalized
        let normalized_password = self.phrase();
        let normalized_salt = {
            let mut salt = Cow::Owned(format!("mnemonic{}", passphrase.as_ref()));
            normalize_utf8(&mut salt);
            salt
        };

        let mut seed = [0u8; PBKDF2_BYTES];
        pbkdf2::pbkdf2::<Hmac<Sha512>>(
            normalized_password.as_bytes(),
            normalized_salt.as_bytes(),
            PBKDF2_ROUNDS,
            &mut seed,
        );
        seed
    }

    /// Returns the [`Language`] of the mnemonic.
    pub fn lang(&self) -> Language {
        self.lang
    }

    /// Returns the mnemonic phrase as a string slice.
    pub fn phrase(&self) -> &str {
        &self.phrase
    }

    /*
    /// Consume the `Mnemonic` and return the phrase as a `String`.
    pub fn into_phrase(mut self) -> String {
        // Create an empty string and swap values with the mnemonic's phrase.
        // This allows `Mnemonic` to implement `Drop`, while still returning the phrase.
        mem::replace(&mut self.phrase, String::new())
    }
    */

    /// Returns the original entropy of the mnemonic phrase.
    pub fn entropy(&self) -> &[u8] {
        &self.entropy
    }

    /*
    /// Consume the `Mnemonic` and return the entropy as a `Vec<u8>`.
    pub fn into_entropy(mut self) -> Vec<u8> {
        // Create an empty bytes and swap values with the mnemonic's entropy.
        // This allows `Mnemonic` to implement `Drop`, while still returning the entropy.
        mem::replace(&mut self.entropy, Vec::new())
    }
    */
}

///////////////////////////////////////////////////////////////////////////////
// Some helper functions
///////////////////////////////////////////////////////////////////////////////

/// Ensure the content of the `s` is normalized UTF8.
/// Avoid allocation for normalization when there are no special UTF8 characters in the string.
#[inline]
fn normalize_utf8(s: &mut Cow<'_, str>) {
    use unicode_normalization::{is_nfkd_quick, IsNormalized, UnicodeNormalization};
    if is_nfkd_quick(s.as_ref().chars()) != IsNormalized::Yes {
        *s = Cow::Owned(s.as_ref().nfkd().to_string())
    }
}

/// Extract the first `bits` from the `source` byte.
/// Can operate on 8-bit integers only.
const fn checksum(source: u8, bits: usize) -> u8 {
    source >> (BITS_PER_BYTE - bits)
}

/// Extract the left `index` bit from the `source` byte.
/// Can operate on 0-7 integers only.
const fn left_index_bit(source: u8, index: usize) -> bool {
    let mask = 1 << (BITS_PER_BYTE - 1 - index);
    source & mask > 0
}

/// Converts `chunk_size` bits to the integer.
#[inline]
fn bits_to_uint(bits: &[bool], chunk_size: usize) -> usize {
    debug_assert_eq!(bits.len(), chunk_size);
    bits.iter()
        .take(chunk_size)
        .enumerate()
        .map(|(i, bit)| if *bit { 1 << (chunk_size - 1 - i) } else { 0 })
        .sum::<usize>()
}

/// Converts a index to bits.
#[inline]
fn index_to_bits(index: usize, bits: &mut [bool], chunk_size: usize) {
    debug_assert!(index < (2 << chunk_size));
    bits.iter_mut()
        .take(chunk_size)
        .enumerate()
        .for_each(|(i, bit)| *bit = (index >> (chunk_size - 1 - i)) & 1 == 1);
}

#[test]
fn test_left_index_bit() {
    assert_eq!(left_index_bit(0b1111_1111, 0), true);
    assert_eq!(left_index_bit(0b1111_1111, 3), true);
    assert_eq!(left_index_bit(0b1111_1111, 7), true);
    assert_eq!(left_index_bit(0b1111_0111, 0), true);
    assert_eq!(left_index_bit(0b1111_0111, 4), false);
    assert_eq!(left_index_bit(0b0100_0000, 0), false);
    assert_eq!(left_index_bit(0b0100_0000, 1), true);
}

#[test]
fn test_bits_to_uint() {
    assert_eq!(bits_to_uint(&[false; 11], BITS_PER_WORD), 0b000_0000_0000); // 0
    assert_eq!(bits_to_uint(&[true; 11], BITS_PER_WORD), 0b111_1111_1111); // 2047
    let mut bits = [false; 11];
    bits[0] = true;
    bits[1] = true;
    bits[2] = true;
    bits[3] = true;
    bits[4] = true;
    assert_eq!(bits_to_uint(&bits, BITS_PER_WORD), 0b111_1100_0000); //1984

    assert_eq!(bits_to_uint(&[false; 8], BITS_PER_BYTE), 0b0000_0000); // 0
    assert_eq!(bits_to_uint(&[true; 8], BITS_PER_BYTE), 0b1111_1111); // 255
    let mut bits = [false; 8];
    bits[0] = true;
    bits[1] = true;
    bits[2] = true;
    bits[3] = true;
    bits[4] = true;
    assert_eq!(bits_to_uint(&bits, BITS_PER_BYTE), 0b1111_1000); //248
}

#[test]
fn test_index_to_bits() {
    let mut bits: [bool; BITS_PER_WORD] = Default::default();
    index_to_bits(0b000_0000_0000, &mut bits, BITS_PER_WORD);
    assert_eq!(bits, [false; BITS_PER_WORD]); // 0

    let mut bits: [bool; BITS_PER_WORD] = Default::default();
    index_to_bits(0b111_1111_1111, &mut bits, BITS_PER_WORD);
    assert_eq!(bits, [true; BITS_PER_WORD]); // 2047

    let mut bits: [bool; BITS_PER_WORD] = Default::default();
    index_to_bits(0b111_1100_0000, &mut bits, BITS_PER_WORD);
    let mut expected_bits = [false; BITS_PER_WORD];
    expected_bits[0] = true;
    expected_bits[1] = true;
    expected_bits[2] = true;
    expected_bits[3] = true;
    expected_bits[4] = true;
    assert_eq!(bits, expected_bits); // 1984
}

#[test]
fn test_mnemonic_word_count() {
    let mnemonic = Count::Words12;
    assert_eq!(mnemonic.word_count(), 12);
    assert_eq!(mnemonic.total_bits(), 128 + 4);
    assert_eq!(mnemonic.entropy_bits(), 128);
    assert_eq!(mnemonic.checksum_bits(), 4);

    let mnemonic = Count::Words15;
    assert_eq!(mnemonic.word_count(), 15);
    assert_eq!(mnemonic.total_bits(), 160 + 5);
    assert_eq!(mnemonic.entropy_bits(), 160);
    assert_eq!(mnemonic.checksum_bits(), 5);

    let mnemonic = Count::Words18;
    assert_eq!(mnemonic.word_count(), 18);
    assert_eq!(mnemonic.total_bits(), 192 + 6);
    assert_eq!(mnemonic.entropy_bits(), 192);
    assert_eq!(mnemonic.checksum_bits(), 6);

    let mnemonic = Count::Words21;
    assert_eq!(mnemonic.word_count(), 21);
    assert_eq!(mnemonic.total_bits(), 224 + 7);
    assert_eq!(mnemonic.entropy_bits(), 224);
    assert_eq!(mnemonic.checksum_bits(), 7);

    let mnemonic = Count::Words24;
    assert_eq!(mnemonic.word_count(), 24);
    assert_eq!(mnemonic.total_bits(), 256 + 8);
    assert_eq!(mnemonic.entropy_bits(), 256);
    assert_eq!(mnemonic.checksum_bits(), 8);
}

#[test]
fn test_mnemonic_zeroize_when_drop() {
    let p: *const String;
    let e: *const Vec<u8>;
    {
        // phrase = "absurd amount doctor acoustic avoid letter advice cage absurd amount doctor adjust"
        // entropy = [1u8; 16]
        let m = Mnemonic::from_entropy([1u8; 16]).unwrap();
        p = &m.phrase;
        e = &m.entropy;
        unsafe {
            println!("*p: {}", (*p));
            println!("*e: {:?}", (*e));
        }
    }

    unsafe {
        assert_ne!(
            (*p),
            "absurd amount doctor acoustic avoid letter advice cage absurd amount doctor adjust"
        );
        println!("*p: {}", (*p));
        assert_ne!((*e), [1u8; 16]);
        println!("*e: {:?}", (*e));
    }
}

/*
#[test]
fn test_mnemonic_consume() {
    {
        let m = Mnemonic::from_entropy([1u8; 16]).unwrap();
        let p: *const String = &m.phrase;
        unsafe {
            println!("*p: {} ({:p})", (*p), p);
        }
        let phrase = m.into_phrase();
        println!("phrase: {} ({:p})", phrase, &phrase);
        unsafe {
            println!("*p: {} ({:p})", (*p), p);
        }
    }

    {
        let m = Mnemonic::from_entropy([1u8; 16]).unwrap();
        let e: *const Vec<u8> = &m.entropy;
        unsafe {
            println!("*e: {:?} ({:p})", (*e), e);
        }
        let entropy = m.into_entropy();
        println!("entropy: {:?} ({:p})", entropy, &entropy);
        unsafe {
            println!("*e: {:?} ({:p})", (*e), e);
        }
    }
}
*/
