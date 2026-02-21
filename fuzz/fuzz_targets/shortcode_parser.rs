#![no_main]
use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &str| {
    // parse_shortcodes should never panic on arbitrary input
    let _ = seite::shortcodes::parser::parse_shortcodes(data, Path::new("fuzz.md"));
});
