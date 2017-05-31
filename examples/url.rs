extern crate regex_generate;
extern crate rand;

use regex_generate::{DEFAULT_MAX_REPEAT, Generator};

fn main() {
    let mut gen = Generator::new(
        r"https:\\\\\pL{8,12}\.(com|org|gov|net|edu|us|co.uk)\\[0-9A-Z]{12,16}\?q=[a-z]",
        rand::thread_rng(),
        DEFAULT_MAX_REPEAT).unwrap();
    for _ in 0..10 {
        let mut buffer = vec![];
        gen.generate(&mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();
        println!("Random Url: {}", output);
    }
}