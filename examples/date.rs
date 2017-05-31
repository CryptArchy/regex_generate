extern crate regex_generate;
extern crate rand;

use regex_generate::{DEFAULT_MAX_REPEAT, Generator};

fn main() {
    let mut gen = Generator::new(r"(?x)
(?P<year>[0-9]{4})  # the year
-
(?P<month>[0-9]{2}) # the month
-
(?P<day>[0-9]{2})   # the day
", rand::thread_rng(), DEFAULT_MAX_REPEAT).unwrap();
    let mut buffer = vec![];
    gen.generate(&mut buffer).unwrap();
    let output = String::from_utf8(buffer).unwrap();

    println!("Random Date: {}", output);
}