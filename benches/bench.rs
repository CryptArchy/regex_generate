#![feature(test)]

extern crate test;
extern crate regex_generate;

const RAND_BENCH_N: u64 = 1000;

use test::{black_box, Bencher};
use regex_generate::{Generate, Generator};

fn test_generate(raw: &str, b: &mut Bencher) {
    let g = Generator::new(raw).unwrap();
    let mut buffer = vec![];

    b.iter(|| {
        for _ in 0..RAND_BENCH_N {
            black_box(g.generate(&mut buffer)).unwrap();
            let buf = buffer.clone();
            black_box(String::from_utf8(buf)).unwrap();
            buffer.clear();
        }
    });
}

#[bench]
fn gen_empty(b: &mut Bencher) {
    test_generate(r"", b);
}

#[bench]
fn gen_any(b: &mut Bencher) {
    test_generate(r".{10}", b);
}

#[bench]
fn gen_literal(b: &mut Bencher) {
    test_generate(r"aBcDe", b);
}

#[bench]
fn gen_alternate(b: &mut Bencher) {
    test_generate(r"(a|b)|(c|d)|(e|f)", b);
}

#[bench]
fn gen_class(b: &mut Bencher) {
    test_generate(r"\p{L}{10}", b);
}

#[bench]
fn gen_not_class(b: &mut Bencher) {
    test_generate(r"\P{L}{10}", b);
}