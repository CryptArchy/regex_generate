// `error_chain!` can recurse deeply
#![recursion_limit = "1024"]
#[macro_use] extern crate error_chain;
extern crate rand;
extern crate regex_syntax;

mod errors;

use errors::*;
use std::io;
use std::ops::{Add, Sub, AddAssign};
use rand::Rng;
use rand::distributions::uniform::{Uniform, SampleUniform};
use rand::seq::SliceRandom;
use regex_syntax::hir::{self, Hir, HirKind};
use regex_syntax::Parser;

pub const DEFAULT_MAX_REPEAT: u32 = 100;

/// Generator reads a string of regular expression syntax and generates strings based on it.
pub struct Generator<R: Rng> {
    hir: Hir,
    rng: R,
    max_repeat: u32,
}

impl<R: Rng> Generator<R> {
    /// Create a new Generator from the regular expression string and use the given Rng for randomization.
    pub fn parse(s: &str, rng: R) -> Result<Generator<R>> {
        Self::new(s, rng, DEFAULT_MAX_REPEAT)
    }

    /// Create a new Generator from the regular expression string and use the given Rng for randomization
    /// with a maximum limit on repititions of the given amount.
    pub fn new(s: &str, rng: R, max_repeat: u32) -> Result<Generator<R>> {
        let hir = Parser::new().parse(s).chain_err(|| "could not parse expression")?;
        Ok(Generator {
            hir: hir,
            rng: rng,
            max_repeat: max_repeat,
        })
    }

    /// Fill the provided buffer with values randomly derived from the regular expression
    pub fn generate<W:io::Write>(&mut self, buffer: &mut W) -> Result<()> {
        Self::generate_from_hir(buffer, &self.hir, &mut self.rng, self.max_repeat)
    }

    fn generate_from_hir<W:io::Write>(buffer: &mut W, hir: &Hir, rng: &mut R, max_repeat: u32) -> Result<()> {
        fn write_char<W: io::Write>(c:char, buffer: &mut W) -> io::Result<()> {
            let mut b = [0; 4];
            let sl = c.encode_utf8(&mut b).len();
            buffer.write(&b[0..sl])?;
            Ok(())
        }

        match *hir.kind() {
            HirKind::Anchor(hir::Anchor::EndLine) => {
                buffer.write(b"\n").chain_err(|| "failed to write end of line")?;
                Ok(())
            }
            HirKind::Empty | HirKind::Anchor(_) | HirKind::WordBoundary(_) => {
                Ok(())
            }
            HirKind::Literal(hir::Literal::Unicode(c)) => {
                write_char(c, buffer).chain_err(|| "failed to write literal value")
            }
            HirKind::Literal(hir::Literal::Byte(b)) => {
                buffer.write(&[b]).chain_err(|| "failed to write literal value")?;
                Ok(())
            }
            HirKind::Class(hir::Class::Unicode(ref class)) => {
                loop {
                    let val = sample_from_ranges(class.ranges(), rng);
                    if let Some(c) = std::char::from_u32(val) {
                        return write_char(c, buffer).chain_err(|| "failed to write class");
                    }
                }
            }
            HirKind::Class(hir::Class::Bytes(ref class)) => {
                let b = sample_from_ranges(class.ranges(), rng) as u8;
                buffer.write(&[b]).chain_err(|| "failed to write class")?;
                Ok(())
            }
            HirKind::Repetition(ref repetition) => {
                let limit = max_repeat - 1;
                let range = match repetition.kind {
                    hir::RepetitionKind::ZeroOrOne => (0, 1),
                    hir::RepetitionKind::ZeroOrMore => (0, limit),
                    hir::RepetitionKind::OneOrMore => (1, limit),
                    hir::RepetitionKind::Range(ref r) => match *r {
                        hir::RepetitionRange::Exactly(n) => (n, n),
                        hir::RepetitionRange::AtLeast(n) => (n, limit),
                        hir::RepetitionRange::Bounded(m, n) => (m, n),
                    },
                };
                let count = if repetition.greedy {
                    rng.sample(Uniform::new_inclusive(range.0, range.1))
                } else {
                    range.0
                };
                for _ in 0..count {
                    Self::generate_from_hir(buffer, &repetition.hir, rng, max_repeat).expect("Fail");
                }
                Ok(())
            }
            HirKind::Group(ref group) => {
                Self::generate_from_hir(buffer, &group.hir, rng, max_repeat)
            }
            HirKind::Concat(ref hirs) => {
                for h in hirs {
                    Self::generate_from_hir(buffer, h, rng, max_repeat).expect("Fail");
                }
                Ok(())
            }
            HirKind::Alternation(ref hirs) => {
                let h = hirs.choose(rng).expect("non empty alternations");
                Self::generate_from_hir(buffer, h, rng, max_repeat)
            }
        }
    }
}

trait Interval {
    type Item: SampleUniform
        + Add<Output = Self::Item>
        + Sub<Output = Self::Item>
        + AddAssign
        + From<u8>
        + Copy
        + Ord;
    fn bounds(&self) -> (Self::Item, Self::Item);
}

impl Interval for hir::ClassUnicodeRange {
    type Item = u32;
    fn bounds(&self) -> (Self::Item, Self::Item) { (self.start().into(), self.end().into()) }
}

impl Interval for hir::ClassBytesRange {
    type Item = u8;
    fn bounds(&self) -> (Self::Item, Self::Item) { (self.start(), self.end()) }
}

const SAMPLE_UNBIASED_LIMIT: usize = 2;

fn sample_from_ranges<I: Interval, R: Rng>(ranges: &[I], rng: &mut R) -> I::Item {
    if ranges.len() <= SAMPLE_UNBIASED_LIMIT {
        // We use unbiased sampling when number of ranges is small.
        // In particular this includes the case of `.` (AnyCharNoNL),
        // which is equivalent to `[\u{0}-\u{9}\u{b}-\u{10ffff}]`.
        // Using the biased sample will give \u{0}-\u{9} 50% of the time and is unrealistic.

        let zero = I::Item::from(0);
        let mut normalized_ranges = [(zero, zero); SAMPLE_UNBIASED_LIMIT];
        let mut normalized_len = zero;
        for (i, r) in ranges.iter().enumerate() {
            let (start, end) = r.bounds();
            normalized_ranges[i] = (normalized_len, start);
            normalized_len += end - start + I::Item::from(1);
        }

        let normalized_index = rng.gen_range(zero..normalized_len);
        let range_index = normalized_ranges[..ranges.len()]
            .binary_search_by(|&(ns, _)| ns.cmp(&normalized_index))
            .unwrap_or_else(|i| i - 1);
        let (normalized_start, start) = normalized_ranges[range_index];

        normalized_index - normalized_start + start

    } else {
        // We use biased sampling otherwise due to speed concern.
        let range = ranges.choose(rng).expect("at least one range in the class");
        let (start, end) = range.bounds();
        rng.sample(Uniform::new_inclusive(start, end))
    }
}

#[cfg(test)]
mod tests {
    extern crate regex;

    use super::{DEFAULT_MAX_REPEAT, Generator};
    use self::regex::Regex;
    use rand;

    const TEST_N: u64 = 10000;

    fn test_regex(raw: &str) {
        let mut gen = Generator::new(raw, rand::thread_rng(), DEFAULT_MAX_REPEAT).unwrap();
        // let expr = Expr::parse(raw).unwrap();
        let rx = Regex::new(raw).unwrap();
        // println!("Testing: {:?} against \\{:?}\\", gen, rx);
        let mut buffer = vec![];

        for _ in 0..TEST_N {
            gen.generate(&mut buffer).unwrap();
            let b = buffer.clone();
            // let s = String::from_utf8_lossy(&b);
            // assert!(rx.is_match(&s), "Unexpected: {:?} on {:?}", s, raw);
            match String::from_utf8(b) {
                Ok(s) => assert!(rx.is_match(&s), "Unexpected: {:?} on {:?}", s, raw),
                Err(err) => assert!(false, "Error: {:?} {:?}", err, raw),
            }
            buffer.clear();
        }
    }

    #[test]
    fn gen_empty() {
        test_regex(r"");
    }

    #[test]
    fn gen_start_end_text() {
        test_regex(r"^a$");
    }

    #[test]
    fn gen_start_end_text_empty() {
        test_regex(r"^$");
    }

    #[test]
    fn gen_start_end_text_always() {
        test_regex(r"\Aa\z");
    }

    #[test]
    fn gen_start_end_line() {
        test_regex(r"(?m)^a$");
    }

    #[test]
    fn gen_word_boundary() {
        test_regex(r"\ba\b b");
    }

    #[test]
    fn gen_not_word_boundary() {
        test_regex(r"a\Bb");
    }

    #[test]
    fn gen_any() {
        test_regex(r"(?s).");
    }

    #[test]
    fn gen_any_no_newline() {
        test_regex(r".");
    }

    #[test]
    fn gen_literal() {
        test_regex(r"aBcDe");
    }

    #[test]
    fn gen_class() {
        test_regex(r"[a-zA-Z0-9]");
    }

    #[test]
    fn gen_repeat_zero_or_one() {
        test_regex(r"a?");
    }

    #[test]
    fn gen_repeat_zero_or_more() {
        test_regex(r"a*");
    }

    #[test]
    fn gen_repeat_one_or_more() {
        test_regex(r"a+");
    }

    #[test]
    fn gen_repeat_range() {
        test_regex(r"a{3,8}");
    }

    #[test]
    fn gen_repeat_range_exact() {
        test_regex(r"a{3}");
    }

    #[test]
    fn gen_repeat_range_open() {
        test_regex(r"a{3,}");
    }

    #[test]
    fn gen_repeat_zero_or_one_lazy() {
        test_regex(r"a??");
    }

    #[test]
    fn gen_repeat_zero_or_more_lazy() {
        test_regex(r"a*?");
    }

    #[test]
    fn gen_repeat_one_or_more_lazy() {
        test_regex(r"a+?");
    }

    #[test]
    fn gen_repeat_range_lazy() {
        test_regex(r"a{3,8}?");
    }

    #[test]
    fn gen_repeat_range_exact_lazy() {
        test_regex(r"a{3}?");
    }

    #[test]
    fn gen_repeat_range_open_lazy() {
        test_regex(r"a{3,}?");
    }

    #[test]
    fn gen_group() {
        test_regex(r"(abcde)");
    }

    #[test]
    fn gen_concat() {
        test_regex(r"a?b?");
    }

    #[test]
    fn gen_alternate() {
        test_regex(r"a|b");
    }

    #[test]
    fn gen_ascii_classes() {
        test_regex(r"[[:alnum:]]");
        test_regex(r"[[:alpha:]]");
        test_regex(r"[[:ascii:]]");
        test_regex(r"[[:cntrl:]]");
        test_regex(r"[[:digit:]]");
        test_regex(r"[[:lower:]]");
        test_regex(r"[[:print:]]");
        test_regex(r"[[:punct:]]");
        test_regex(r"[[:space:]]");
        test_regex(r"[[:upper:]]");
        test_regex(r"[[:word:]]");
        test_regex(r"[[:xdigit:]]");
    }

    #[test]
    fn gen_perl_classes() {
        test_regex(r"\d+");
        test_regex(r"\D+");
        test_regex(r"\s+");
        test_regex(r"\S+");
        test_regex(r"\w+");
        test_regex(r"\W+");
    }

    #[test]
    fn gen_unicode_classes() {
        test_regex(r"\p{L}");
        test_regex(r"\P{L}");
        test_regex(r"\p{M}");
        test_regex(r"\P{M}");
        test_regex(r"\p{N}");
        test_regex(r"\P{N}");
        test_regex(r"\p{P}");
        test_regex(r"\P{P}");
        test_regex(r"\p{S}");
        test_regex(r"\P{S}");
        test_regex(r"\p{Z}");
        test_regex(r"\P{Z}");
        test_regex(r"\p{C}");
        test_regex(r"\P{C}");
    }

    #[test]
    fn gen_unicode_script_classes() {
        test_regex(r"\p{Latin}");
        test_regex(r"\p{Greek}");
        test_regex(r"\p{Cyrillic}");
        test_regex(r"\p{Armenian}");
        test_regex(r"\p{Hebrew}");
        test_regex(r"\p{Arabic}");
        test_regex(r"\p{Syriac}");
        test_regex(r"\p{Thaana}");
        test_regex(r"\p{Devanagari}");
        test_regex(r"\p{Bengali}");
        test_regex(r"\p{Gurmukhi}");
        test_regex(r"\p{Gujarati}");
        test_regex(r"\p{Oriya}");
        test_regex(r"\p{Tamil}");
        test_regex(r"\p{Hangul}");
        test_regex(r"\p{Hiragana}");
        test_regex(r"\p{Katakana}");
        test_regex(r"\p{Han}");
        test_regex(r"\p{Tagalog}");
        test_regex(r"\p{Linear_B}");
        test_regex(r"\p{Inherited}");
    }

    #[test]
    fn gen_complex() {
        test_regex(r"^(\p{Greek}\P{Greek})(?:\d{3,6})$");
    }
}
