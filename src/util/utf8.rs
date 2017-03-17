//! UTF-8 Validation for a byte stream.
use std::error::Error;
use std::fmt;
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq)]
/// UTF-8 Validation Errors
pub enum UTF8Error {
    /// The code point is larger thant the maximum allowed (U+10FFFF)
    MaxiumuCodePoint,
    /// The 2nd-byte in a 2-byte sequence in not a valid continuation sequence.
    TwoByteContinuation,
    /// Found a 2-byte sequence that could be represented as 1-byte.
    TwoByteOverlong,
    /// The xth-byte in a 3-byte sequence is not a valid continuation sequence.
    ThreeByteContinuation(u8),
    /// Found a 3-byte sequence that could be represented as 2 or 1-byte.
    ThreeByteOverlong,
    /// The xth-byte in a 4-byte sequence is not a valid continuation sequence.
    FourByteContinuation(u8),
    /// Found a 4-byte sequence that could be represented as 3, 2 or 1-byte.
    FourByteOverlong,
    /// Found an invalid first byte (0x80-0xbf)
    InvalidFirstByte(u8),
}

impl fmt::Display for UTF8Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            UTF8Error::MaxiumuCodePoint => {
                write!(f,
                       "The code point is larger thant the maximum allowed (U+10FFFF)")
            }
            UTF8Error::TwoByteContinuation => {
                write!(f,
                       "The 2nd-byte in a 2-byte sequence in not a valid continuation sequence.")
            }
            UTF8Error::TwoByteOverlong => {
                write!(f,
                       "Found a 2-byte sequence that could be represented as 1-byte.")
            }
            UTF8Error::ThreeByteContinuation(ref b) => {
                write!(f,
                       "The {}th-byte in a 3-byte sequence is not a valid continuation sequence.",
                       b)
            }
            UTF8Error::ThreeByteOverlong => {
                write!(f,
                       "Found a 3-byte sequence that could be represented as 2 or 1-byte.")
            }
            UTF8Error::FourByteContinuation(ref b) => {
                write!(f,
                       "The {}th-byte in a 4-byte sequence is not a valid continuation sequence.",
                       b)
            }
            UTF8Error::FourByteOverlong => {
                write!(f,
                       "Found a 4-byte sequence that could be represented as 3, 2 or 1-byte.")
            }
            UTF8Error::InvalidFirstByte(ref b) => {
                write!(f, "Found an invalid first byte (0x80-0xbf): {}", b)
            }
        }
    }
}

impl Error for UTF8Error {
    fn description(&self) -> &str {
        match *self {
            UTF8Error::MaxiumuCodePoint => {
                "The code point is larger thant the maximum allowed \
            (U+10FFFF)"
            }
            UTF8Error::TwoByteContinuation => {
                "The 2nd-byte in a 2-byte sequence in not a valid \
            continuation sequence."
            }
            UTF8Error::TwoByteOverlong => {
                "Found a 2-byte sequence that could be represented as \
            1-byte."
            }
            UTF8Error::ThreeByteContinuation(_) => {
                "The xth-byte in a 3-byte sequence is not a \
            valid continuation sequence."
            }
            UTF8Error::ThreeByteOverlong => {
                "Found a 3-byte sequence that could be represented as \
            2 or 1-byte."
            }
            UTF8Error::FourByteContinuation(_) => {
                "The xth-byte in a 4-byte sequence is not a \
            valid continuation sequence."
            }
            UTF8Error::FourByteOverlong => {
                "Found a 4-byte sequence that could be represented as \
            3, 2 or 1-byte."
            }
            UTF8Error::InvalidFirstByte(_) => "Found an invalid first byte (0x80-0xbf)",
        }
    }
}

/// Returns true if the given byte doesn't start with 10xxxxxx.
fn doesnt_start_with_10(byte: u8) -> bool {
    byte & 0xc0 != 0x80
}

/// Print out the validation duration.
fn perf(_len: usize, _dur: Duration) {
    // stdout_trace!("Validate buf of len {} in {}.{}s",
    //               len,
    //               dur.as_secs(),
    //               dur.subsec_nanos());
}

/// Validate that the given buffer is valid UTF-8.   If we reach the end of the buffer, and it is
/// still valid then return `Ok(None)` to wait for more bytes from poll.
pub fn validate(buf: &[u8]) -> Result<Option<()>, UTF8Error> {
    use util::utf8::UTF8Error::*;
    let now = Instant::now();
    let len = buf.len();
    let mut iter = buf.iter();
    let mut next = iter.next();

    while let Some(byte) = next {
        // This indicates a 4-byte sequence out of maximum code point range.
        if *byte > 0xf4 {
            perf(len, now.elapsed());
            return Err(MaxiumuCodePoint);
        }
        // Skip over 1-byte codes 0xxxxxxx
        // 7-bit code points
        else if *byte < 0x80 {
            next = iter.next();
        }
        // Handle the 2-byte codes 110 xxxxx 10 xxxxxx
        // 11-bit code points
        else if *byte & 0xe0 == 0xc0 {
            if let Some(second_byte) = iter.next() {
                if doesnt_start_with_10(*second_byte) {
                    perf(len, now.elapsed());
                    return Err(TwoByteContinuation);
                }
                // Check for 2-byte overlong sequence
                else if *byte & 0xfe == 0xc0 {
                    perf(len, now.elapsed());
                    return Err(TwoByteOverlong);
                } else {
                    next = iter.next();
                }
            } else {
                /// We need more bytes,  return Ok(None)
                perf(len, now.elapsed());
                return Ok(None);
            }
        }
        // Handle the 3-byte codes 1110 xxxx 10 xxxxxx 10 xxxxxx
        // 16-bit code points
        else if *byte & 0xf0 == 0xe0 {
            if let Some(second_byte) = iter.next() {
                if let Some(third_byte) = iter.next() {
                    // If the second byte doesn't start with 10 error out.
                    if doesnt_start_with_10(*second_byte) {
                        perf(len, now.elapsed());
                        return Err(ThreeByteContinuation(2));
                    }
                    // If the third byte doesn't start with 10 error out.
                    else if doesnt_start_with_10(*third_byte) {
                        perf(len, now.elapsed());
                        return Err(ThreeByteContinuation(3));
                    }
                    // Check for 3-byte overlong condition
                    // UTF-16 surrogates
                    else if (*byte == 0xe0 && (*second_byte & 0xe0 == 0x80)) ||
                              (*byte == 0xed && (*second_byte & 0xe0 == 0xa0)) {
                        perf(len, now.elapsed());
                        return Err(ThreeByteOverlong);
                    } else {
                        next = iter.next();
                    }
                } else {
                    /// We need more bytes,  return Ok(None)
                    perf(len, now.elapsed());
                    return Ok(None);
                }
            } else {
                /// We need more bytes,  return Ok(None)
                perf(len, now.elapsed());
                return Ok(None);
            }
        }
        // Handle the 4-bytes codes 11110 xxx 10 xxxxxx 10 xxxxxx 10 xxxxxx
        // 21-bit code points
        else if *byte & 0xf8 == 0xf0 {
            if let Some(second_byte) = iter.next() {
                if *byte == 0xf4 && *second_byte > 0x8f {
                    perf(len, now.elapsed());
                    return Err(MaxiumuCodePoint);
                } else if let Some(third_byte) = iter.next() {
                    if let Some(fourth_byte) = iter.next() {
                        if doesnt_start_with_10(*second_byte) {
                            perf(len, now.elapsed());
                            return Err(FourByteContinuation(2));
                        } else if doesnt_start_with_10(*third_byte) {
                            perf(len, now.elapsed());
                            return Err(FourByteContinuation(3));
                        } else if doesnt_start_with_10(*fourth_byte) {
                            perf(len, now.elapsed());
                            return Err(FourByteContinuation(4));
                        } else if *byte == 0xf0 && (*second_byte & 0xf0 == 0x80) {
                            perf(len, now.elapsed());
                            return Err(FourByteOverlong);
                        } else {
                            next = iter.next();
                        }
                    } else {
                        /// We need more bytes,  return Ok(None)
                        perf(len, now.elapsed());
                        return Ok(None);
                    }
                } else {
                    /// We need more bytes,  return Ok(None)
                    perf(len, now.elapsed());
                    return Ok(None);
                }
            } else {
                /// We need more bytes,  return Ok(None)
                perf(len, now.elapsed());
                return Ok(None);
            }
        } else {
            // This covers 1-byte 0x80 - 0xbf
            perf(len, now.elapsed());
            return Err(InvalidFirstByte(*byte));
        }
    }

    perf(len, now.elapsed());
    Ok(Some(()))
}

#[cfg(test)]
mod test {
    use super::{validate, UTF8Error};

    // Smallest 1-byte (U+0000)
    const V1: [u8; 1] = [0x00];
    // Largest valid 1-byte (U+007F)
    const V3: [u8; 1] = [0x7F];
    // Smallest 2-byte. (U+0080)
    // Smallest 11-bits code point 1000 0000 (U+0080), that won't fit in 7 bits.
    // 110 00000 10 000000
    const V4: [u8; 2] = [0xc2, 0x80];
    // Largest 2-byte. (U+07FF)
    // Largest 11-bits code point 11111111111
    // 110 11111 10 111111
    const V5: [u8; 2] = [0xdf, 0xbf];
    // Smallest 3-byte (U+0800)
    // Smallest 16-bits code-point 00001000 00000000 (U+0800), that won't fit in 11 bits.
    // 1110 0000 10 100000 10 000000
    const V6: [u8; 3] = [0xe0, 0xa0, 0x80];
    // Largest 3-byte (U+FFFF)
    // Largest 16-bits code-point 11111111 11111111 (U+FFFF)
    // 1110 1111 10 111111 10 111111
    const V7: [u8; 3] = [0xef, 0xbf, 0xbf];
    // Smallest 4-byte. (U+10000)
    // Smallest 21-bits code-point 00001 00000000 00000000 (U+10000), that won't fit in 16 bits.
    // 11110 000 10 010000 10 000000 10 000000
    const V8: [u8; 4] = [0xf0, 0x90, 0x80, 0x80];
    // Largest 4-byte, should return Ok(Some())
    // Largest 21-bits code point 10000 11111111 11111111 (U+10FFFF), allowed by spec.
    // 11110 100 10 001111 10 111111 10 111111
    const V9: [u8; 4] = [0xf4, 0x8f, 0xbf, 0xbf];

    // Partial UTF-8, should return Ok(None)
    #[cfg_attr(rustfmt, rustfmt_skip)]
    const PARTIAL_UTF_8: [u8; 12] =
        [0xce, 0xba, 0xe1, 0xbd,
         0xb9, 0xcf, 0x83, 0xce,
         0xbc, 0xce, 0xb5, 0xf4];

    // Invalid continuation byte. Must be 0x80 - 0xbf.
    const C1: [u8; 2] = [0xc0, 0x7f];
    const C2: [u8; 2] = [0xc0, 0xc0];
    const C3: [u8; 3] = [0xe0, 0x7f, 0x80];
    const C4: [u8; 3] = [0xe0, 0xc0, 0x80];
    const C5: [u8; 3] = [0xe0, 0x80, 0x7f];
    const C6: [u8; 3] = [0xe0, 0x80, 0xc0];
    const C7: [u8; 4] = [0xf0, 0x7f, 0x80, 0x80];
    const C8: [u8; 4] = [0xf0, 0xc0, 0x80, 0x80];
    const C9: [u8; 4] = [0xf0, 0x80, 0x7f, 0x80];
    const C10: [u8; 4] = [0xf0, 0x80, 0xc0, 0x80];
    const C11: [u8; 4] = [0xf0, 0x80, 0x80, 0x7f];
    const C12: [u8; 4] = [0xf0, 0x80, 0x80, 0xc0];

    // Smallest overlong 2-byte.
    // 110 00000 10 000000 (U+0000)
    const O1: [u8; 2] = [0xc0, 0x80];
    // Largest overlong 2-byte.
    // 110 00001 10 111111 (U+007F)
    const O2: [u8; 2] = [0xc1, 0xbf];
    // Smallest 3-byte overlong.
    // 1110 0000 10 000000 10 000000 (U+0000)
    const O3: [u8; 3] = [0xe0, 0x80, 0x80];
    // Larget 3-byte overlong.
    // 1110 0000 10 011111 10 111111 (U+07FF)
    const O4: [u8; 3] = [0xe0, 0x9f, 0xbf];
    // Smallest 4-byte overlong.
    // 11110 000 10 000000 10 000000 10 000000 (U+0000)
    const O5: [u8; 4] = [0xf0, 0x80, 0x80, 0x80];
    // Largest 4-byte overlong
    // 11110 000 10 00 1111 10 111111 10 111111 (U+FFFF)
    const O6: [u8; 4] = [0xf0, 0x8f, 0xbf, 0xbf];

    // Maximum code point exceeded.
    const M1: [u8; 4] = [0xf4, 0x90, 0xbf, 0xbf];

    // Partial UTF-8, should return Ok(None)
    #[cfg_attr(rustfmt, rustfmt_skip)]
    const INVALID_UTF_8: [u8; 13] =
        [0xce, 0xba, 0xe1, 0xbd,
         0xb9, 0xcf, 0x83, 0xce,
         0xbc, 0xce, 0xb5, 0xf4,
         0x90];

    #[test]
    fn valid_one_byte_utf8() {
        let valids = [V1, V3];

        for (idx, valid) in valids.iter().enumerate() {
            if let Ok(Some(_)) = validate(valid) {
                assert!(true);
            } else {
                println!("valid as index {} is invalid!", idx);
                assert!(false);
            }
        }
    }

    #[test]
    fn valid_two_byte_utf8() {
        let valids = [V4, V5];

        for (idx, valid) in valids.iter().enumerate() {
            if let Ok(Some(_)) = validate(valid) {
                assert!(true);
            } else {
                println!("valid as index {} is invalid!", idx);
                assert!(false);
            }
        }
    }

    #[test]
    fn valid_three_byte_utf8() {
        let valids = [V6, V7];

        for (idx, valid) in valids.iter().enumerate() {
            if let Ok(Some(_)) = validate(valid) {
                assert!(true);
            } else {
                println!("valid as index {} is invalid!", idx);
                assert!(false);
            }
        }
    }

    #[test]
    fn valid_four_byte_utf8() {
        let valids = [V8, V9];

        for (idx, valid) in valids.iter().enumerate() {
            if let Ok(Some(_)) = validate(valid) {
                assert!(true);
            } else {
                println!("valid as index {} is invalid!", idx);
                assert!(false);
            }
        }
    }

    #[test]
    fn invalid_continuation_two_byte_utf8() {
        let invalids = [C1, C2];

        for (idx, invalid) in invalids.iter().enumerate() {
            if let Err(e) = validate(invalid) {
                assert!(e == UTF8Error::TwoByteContinuation);
            } else {
                println!("Two byte continuation at {} didn't error", idx);
                assert!(false);
            }
        }
    }

    #[test]
    fn invalid_continuation_three_byte_utf8() {
        let second_byte_invalids = [C3, C4];
        let third_byte_invalids = [C5, C6];

        for (idx, invalid) in second_byte_invalids.iter().enumerate() {
            if let Err(e) = validate(invalid) {
                assert!(e == UTF8Error::ThreeByteContinuation(2));
            } else {
                println!("Two byte continuation at {} didn't error", idx);
                assert!(false);
            }
        }

        for (idx, invalid) in third_byte_invalids.iter().enumerate() {
            if let Err(e) = validate(invalid) {
                assert!(e == UTF8Error::ThreeByteContinuation(3));
            } else {
                println!("Two byte continuation at {} didn't error", idx);
                assert!(false);
            }
        }
    }

    #[test]
    fn invalid_continuation_four_byte_utf8() {
        let second_byte_invalids = [C7, C8];
        let third_byte_invalids = [C9, C10];
        let fourth_byte_invalids = [C11, C12];

        for (idx, invalid) in second_byte_invalids.iter().enumerate() {
            if let Err(e) = validate(invalid) {
                assert!(e == UTF8Error::FourByteContinuation(2));
            } else {
                println!("Two byte continuation at {} didn't error", idx);
                assert!(false);
            }
        }

        for (idx, invalid) in third_byte_invalids.iter().enumerate() {
            if let Err(e) = validate(invalid) {
                assert!(e == UTF8Error::FourByteContinuation(3));
            } else {
                println!("Two byte continuation at {} didn't error", idx);
                assert!(false);
            }
        }

        for (idx, invalid) in fourth_byte_invalids.iter().enumerate() {
            if let Err(e) = validate(invalid) {
                assert!(e == UTF8Error::FourByteContinuation(4));
            } else {
                println!("Two byte continuation at {} didn't error", idx);
                assert!(false);
            }
        }
    }

    #[test]
    fn overlong_two_byte_utf8() {
        let invalids = [O1, O2];

        for (idx, invalid) in invalids.iter().enumerate() {
            if let Err(e) = validate(invalid) {
                assert!(e == UTF8Error::TwoByteOverlong);
            } else {
                println!("Two byte overlong at {} didn't error", idx);
                assert!(false);
            }
        }
    }

    #[test]
    fn overlong_three_byte_utf8() {
        let invalids = [O3, O4];

        for (idx, invalid) in invalids.iter().enumerate() {
            if let Err(e) = validate(invalid) {
                assert!(e == UTF8Error::ThreeByteOverlong);
            } else {
                println!("Three byte overlong at {} didn't error", idx);
                assert!(false);
            }
        }
    }

    #[test]
    fn overlong_four_byte_utf8() {
        let invalids = [O5, O6];

        for (idx, invalid) in invalids.iter().enumerate() {
            if let Err(e) = validate(invalid) {
                assert!(e == UTF8Error::FourByteOverlong);
            } else {
                println!("Four byte overlong at {} didn't error", idx);
                assert!(false);
            }
        }
    }

    #[test]
    fn maximum_code_point() {
        if let Err(e) = validate(&M1) {
            assert!(e == UTF8Error::MaxiumuCodePoint);
        } else {
            println!("Four byte with codepoint >U+10FFFF didn't error");
            assert!(false);
        }

        for val in 0xf5..0xff {
            if let Err(e) = validate(&[val]) {
                assert!(e == UTF8Error::MaxiumuCodePoint);
            } else {
                println!("Four byte with codepoint >U+10FFFF didn't error");
                assert!(false);
            }
        }
    }

    #[test]
    fn invalid_first_byte() {
        for val in 0x80..0xbf {
            if let Err(e) = validate(&[val]) {
                assert!(e == UTF8Error::InvalidFirstByte(val));
            } else {
                println!("0x{:2x} should be an invalid first byte", val);
                assert!(false);
            }
        }
    }

    #[test]
    fn partial_utf8() {
        if let Ok(None) = validate(&PARTIAL_UTF_8) {
            assert!(true);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn invalid_utf8() {
        if let Err(_e) = validate(&INVALID_UTF_8) {
            assert!(true);
        } else {
            assert!(false);
        }
    }
}
