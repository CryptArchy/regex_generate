# regex_generate

Use regular expressions to generate text.
This crate is very new and raw. It's a work-in-progress, but feel free to add
issues or PRs or use it for your own ideas, if you find it interesting.
No guarantees or warranties are implied, use this code at your own risk.

Thanks to the amazing folks who work on rust-lang/regex which is the heart of this crate.
Using regex_syntax made this crate 1000x easier to produce.

## Documentation

Magically generated and graciously hosted by [Docs.rs](https://docs.rs/regex_generate).

The documentation is not good right now.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
regex_generate = "0.2"
```

and this to your crate root:

```rust
extern crate regex_generate;
```

This example generates a date in YYYY-MM-DD format and prints it.
Adapted from the example for rust-lang/regex.

```rust
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
}!("Random Date: {}", output);
```

## Tests

Run tests with `cargo test`

## Benches

Run benchmarks with `rustup run nightly cargo bench`

## Tips

- Be explicit in your character classes or you will get unexpected results.
- `.` really means _any_, as in any valid unicode character.
- Likewise, `\d` means _any_ number, not just `[0-9]`.
- The default maximum for repetitions (like `.*`) is 100, but you can set it yourself with `generate_with_max_repeat`.

## TODO

- [ ] Use a custom error type
- [ ] Write documentation
- [ ] Cleanup uses of `.expect("Fail")`
- [ ] Add convenience method for directly generating complete strings
- [ ] Add tests for regex bytes feature
- [ ] Account for case insensitivity in `Literal`
- [ ] Do something with group numbers or names? (No back referencing in the syntax, so maybe nothing can be done.)

## License

regex_generate is primarily distributed under the terms of both the MIT license and the Apache License (Version 2.0), with portions covered by various BSD-like licenses.

See LICENSE-APACHE and LICENSE-MIT for details.