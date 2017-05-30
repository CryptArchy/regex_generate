extern crate regex_generate;

use regex_generate::{Generate, Generator};

fn main() {
    let gen = Generator::new(r"https:\\\\\pL{8,12}\.(com|org|gov|net|edu)\\[0-9A-Z]{12,16}\?q=[a-z]").unwrap();
    for _ in 0..10 {
        let mut buffer = vec![];
        gen.generate(&mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();
        println!("Random Url: {}", output);
    }
}