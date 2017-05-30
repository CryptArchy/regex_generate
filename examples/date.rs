extern crate regex_generate;

use regex_generate::{Generate, Generator};

fn main() {
    let gen = Generator::new(r"(?x)
(?P<year>[0-9]{4})  # the year
-
(?P<month>[0-9]{2}) # the month
-
(?P<day>[0-9]{2})   # the day
").unwrap();
    let mut buffer = vec![];
    gen.generate(&mut buffer).unwrap();
    let output = String::from_utf8(buffer).unwrap();

    println!("Random Date: {}", output);
}