// `error_chain!` can recurse deeply
#![recursion_limit = "1024"]
#[macro_use] extern crate error_chain;
extern crate byteorder;
extern crate rand;
extern crate regex_syntax;

mod errors;

use errors::*;
use std::io;
use byteorder::WriteBytesExt;
use rand::{Rng};
use regex_syntax::{Expr, Repeater};

pub const DEFAULT_MAX_REPEAT: u32 = 100;
const NEWLINE_U8: u8 = b'\n';
const NEWLINE: char = '\n';

/// Generator reads a string of regular expression syntax and generates strings based on it.
pub struct Generator<R: Rng> {
    expr: Expr,
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
        let expr = Expr::parse(s).chain_err(|| "could not parse expression")?;
        Ok(Generator {
            expr: expr,
            rng: rng,
            max_repeat: max_repeat,
        })
    }

    /// Fill the provided buffer with values randomly derived from the regular expression
    pub fn generate<W:io::Write>(&mut self, buffer: &mut W) -> Result<()> {
        Self::generate_from_expr(buffer, &self.expr, &mut self.rng, self.max_repeat)
    }

    fn generate_from_expr<W:io::Write>(buffer: &mut W, expr: &Expr, rng: &mut R, max_repeat: u32) -> Result<()> {
        fn write_char<W: io::Write>(c:char, buffer: &mut W) -> io::Result<()> {
            let mut b = [0; 4];
            let sl = c.encode_utf8(&mut b).len();
            buffer.write(&b[0..sl])?;
            Ok(())
        }

        match expr {
            &Expr::Empty |
            &Expr::StartText |
            &Expr::EndText |
            &Expr::WordBoundary |
            &Expr::NotWordBoundary |
            &Expr::WordBoundaryAscii |
            &Expr::NotWordBoundaryAscii |
            &Expr::StartLine => Ok(()),
            &Expr::EndLine => { write_char('\n', buffer).chain_err(|| "failed to write end of line") },
            &Expr::AnyChar => {
                let c = rng.gen::<char>();
                write_char(c, buffer).chain_err(|| "failed to write any char")
            },
            &Expr::AnyCharNoNL => {
                let mut c = NEWLINE;
                while c == NEWLINE { c = rng.gen::<char>(); }
                write_char(c, buffer).chain_err(|| "failed to write any char no newline")
            },
            &Expr::Literal{ref chars, casei:_} => {
                let s: String = chars.iter().collect();
                buffer.write(s.as_bytes()).and(Ok(())).chain_err(|| "failed to write literal value")
            },
            &Expr::Class(ref ranges) => {
                let idx = rng.gen_range(0, ranges.len());
                let range = ranges[idx];
                let start:u32 = range.start.into();
                let end:u32 = range.end.into();
                loop {
                    match std::char::from_u32(rng.gen_range(start, end + 1)) {
                        Some(c) => { return write_char(c, buffer).chain_err(|| "failed to write class") },
                        None => continue,
                    }
                }
            },
            &Expr::Group{e: ref exp, i:_, name:_} => Self::generate_from_expr(buffer, exp, rng, max_repeat),
            &Expr::Concat(ref exps) => {
                for exp in exps.iter() {
                    Self::generate_from_expr(buffer, exp, rng, max_repeat).expect("Fail");
                }
                Ok(())
            },
            &Expr::Alternate(ref exps) => {
                let idx = rng.gen_range(0, exps.len());
                let ref exp = exps[idx];
                Self::generate_from_expr(buffer, exp, rng, max_repeat)
            },
            &Expr::Repeat{e: ref exp, r: rep, greedy} => {
                let range = match rep {
                    Repeater::ZeroOrOne => if greedy { 0..2 } else { 0..1 },
                    Repeater::ZeroOrMore => if greedy { 0..max_repeat } else { 0..1 },
                    Repeater::OneOrMore => if greedy { 1..max_repeat } else { 1..2 },
                    Repeater::Range{min, max:None} => if greedy { min..max_repeat } else { min..(min + 1) },
                    Repeater::Range{min, max:Some(max)} => if greedy { min..(max + 1) } else { min..(min + 1) },
                };
                let count = rng.gen_range(range.start, range.end);
                for _ in 0..count {
                    Self::generate_from_expr(buffer, exp, rng, max_repeat).expect("Fail");
                }
                Ok(())
            },
            &Expr::AnyByte => buffer.write_u8(rng.gen::<u8>()).chain_err(|| "failed to write any byte"),
            &Expr::AnyByteNoNL => {
                let mut c = NEWLINE_U8;
                while c == NEWLINE_U8 { c = rng.gen::<u8>(); }
                buffer.write_u8(c).chain_err(|| "failed to write any byte no newline")
            }
            &Expr::ClassBytes(ref ranges) => {
                let idx = rng.gen_range(0, ranges.len());
                let range = ranges[idx];
                buffer.write_u8(rng.gen_range(range.start, range.end + 1))
                    .chain_err(|| "failed to write class bytes")
            }
            &Expr::LiteralBytes{ref bytes, casei:_} => buffer.write(bytes).and(Ok(()))
                .chain_err(|| "failed to write literal bytes"),
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate regex;

    use super::{DEFAULT_MAX_REPEAT, Generator};
    use self::regex::Regex;
    use regex_syntax::Expr;
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
