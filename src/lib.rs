extern crate byteorder;
extern crate rand;
extern crate regex_syntax;

use byteorder::WriteBytesExt;
use rand::{Rng};
use regex_syntax::{Expr, Repeater};
use std::io;

const DEFAULT_MAX_REPEAT: u32 = 100;

/// Generate provides methods to fill a buffer with generated values
pub trait Generate<T: io::Write> {
    /// Fill the buffer with generated values, only performing repetitions up to the given number of times.
    fn generate_with_max_repeat(&self, buffer: &mut T, max_repeat: u32) -> io::Result<()>;
    /// Fill the buffer with generated values.
    fn generate(&self, buffer: &mut T) -> io::Result<()> {
        self.generate_with_max_repeat(buffer, DEFAULT_MAX_REPEAT)
    }
}

/// Generator reads a string of regular expression syntax and generates strings based on it.
pub struct Generator {
    expr: Expr,
}

impl Generator {
    /// Create a new Generator from the regular expression
    pub fn new(s: &str) -> Result<Generator, regex_syntax::Error> {
        let expr = Expr::parse(s)?;
        Ok(Generator {
            expr: expr
        })
    }
}

impl<T: io::Write> Generate<T> for Generator {
    fn generate_with_max_repeat(&self, buffer: &mut T, max_repeat: u32) -> io::Result<()> {
        self.expr.generate_with_max_repeat(buffer, max_repeat)
    }
}

impl<T: io::Write> Generate<T> for Expr {
    fn generate_with_max_repeat(&self, buffer: &mut T, max_repeat: u32) -> io::Result<()> {
        let mut rng = rand::thread_rng();

        fn write_char<T: io::Write>(c:char, buffer: &mut T) {
            let mut b = [0; 4];
            let sl = c.encode_utf8(&mut b).len();
            buffer.write(&b[0..sl]).expect("Fail");
        }

        match self {
            &Expr::Empty |
            &Expr::StartText |
            &Expr::EndText |
            &Expr::WordBoundary |
            &Expr::NotWordBoundary |
            &Expr::WordBoundaryAscii |
            &Expr::NotWordBoundaryAscii |
            &Expr::StartLine => Ok(()),
            &Expr::EndLine => { write_char('\n', buffer); Ok(()) },
            &Expr::AnyChar => {
                let c = rng.gen::<char>();
                write_char(c, buffer);
                Ok(())
            },
            &Expr::AnyCharNoNL => {
                let mut c = '\n';
                while c == '\n' { c = rng.gen::<char>(); }
                write_char(c, buffer);
                Ok(())
            },
            &Expr::Literal{ref chars, casei:_} => {
                let s: String = chars.iter().collect();
                buffer.write(s.as_bytes()).and(Ok(()))
            },
            &Expr::Class(ref ranges) => {
                let idx = rng.gen_range(0, ranges.len());
                let range = ranges[idx];
                let start:u32 = range.start.into();
                let end:u32 = range.end.into();
                loop {
                    match std::char::from_u32(rng.gen_range(start, end + 1)) {
                        Some(c) => { write_char(c, buffer); return Ok(()); },
                        None => continue,
                    }
                }
            },
            &Expr::Group{e: ref exp, i:_, name:_} => exp.generate(buffer),
            &Expr::Concat(ref exps) => {
                for exp in exps.iter() {
                    exp.generate(buffer).expect("Fail");
                }
                Ok(())
            },
            &Expr::Alternate(ref exps) => {
                let idx = rng.gen_range(0, exps.len());
                let ref exp = exps[idx];
                exp.generate(buffer)
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
                    exp.generate(buffer).expect("Fail");
                }
                Ok(())
            },
            &Expr::AnyByte => buffer.write_u8(rng.gen::<u8>()),
            &Expr::AnyByteNoNL => {
                let mut c = 10;
                while c == 10 { c = rng.gen::<u8>(); }
                buffer.write_u8(c)
            }
            &Expr::ClassBytes(ref ranges) => {
                let idx = rng.gen_range(0, ranges.len());
                let range = ranges[idx];
                buffer.write_u8(rng.gen_range(range.start, range.end + 1))
            }
            &Expr::LiteralBytes{ref bytes, casei:_} => buffer.write(bytes).and(Ok(())),
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate regex;

    use super::{Generate, Generator};
    use self::regex::Regex;
    use regex_syntax::Expr;

    const TEST_N: u64 = 10000;

    fn test_regex(raw: &str) {
        let expr = Expr::parse(raw).unwrap();
        let rx = Regex::new(raw).unwrap();
        // println!("Testing: {:?} against \\{:?}\\", gen, rx);
        let mut buffer = vec![];

        for _ in 0..TEST_N {
            expr.generate(&mut buffer).unwrap();
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

    fn test_generator(raw: &str) {
        let g = Generator::new(raw).unwrap();
        let rx = Regex::new(raw).unwrap();
        let mut buffer = vec![];

        for _ in 0..TEST_N {
            g.generate(&mut buffer).unwrap();
            let b = buffer.clone();
            match String::from_utf8(b) {
                Ok(s) => assert!(rx.is_match(&s), "Unexpected: {:?} on {:?}", s, raw),
                Err(err) => assert!(false, "Error: {:?} {:?}", err, raw),
            }
            buffer.clear();
        }
    }

    #[test]
    fn generator() {
        test_generator(r"\p{Latin}");
    }
}
