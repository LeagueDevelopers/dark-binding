// `error_chain` recursion adjustment
#![recursion_limit = "2048"]
// Make rustc's built-in lints more strict (I'll opt back out selectively)
#![warn(warnings)]
#![allow(unused_must_use)]
// (Or at least find a way to enable build-time and `cargo clippy`-time with a single feature)
// Set clippy into a whitelist-based configuration so I'll see new lints as they come in
#![cfg_attr(feature = "cargo-clippy", warn(clippy_pedantic, clippy_restrictions))]
// Opt out of the lints I've seen and don't want
#![cfg_attr(feature = "cargo-clippy", allow(assign_ops, float_arithmetic))]
// Avoid bundling a copy of jemalloc when building on nightly for maximum size reduction
#![cfg_attr(feature = "nightly", feature(alloc_system))]
#[cfg(feature = "nightly")]
extern crate alloc_system;

extern crate regex;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate error_chain;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate reqwest;
extern crate rand;
extern crate websocket;
extern crate tokio_core;
extern crate native_tls;
extern crate systray;

mod errors;
#[macro_use]
mod util;
mod league_client;

use clap::{App, Arg};
use reqwest::{Client, Certificate};

use errors::*;

static CERTIFICATE: &'static [u8] = include_bytes!("../lcu.der");

lazy_static! {
  static ref HTTP_CLIENT: Client = Client::builder().add_root_certificate(Certificate::from_der(CERTIFICATE).unwrap()).build().unwrap();
}

fn main() {
  if let Err(ref e) = run() {
    use std::io::Write;
    let stderr = &mut ::std::io::stderr();
    let stderr_fail_msg = "Error writing to stderr";

    // Write the top-level error message
    writeln!(stderr, "error: {}", e).expect(stderr_fail_msg);

    // Trace back through the chained errors
    for e in e.iter().skip(1) {
      writeln!(stderr, "caused by: {}", e).expect(stderr_fail_msg);
    }

    // Print the backtrace if available
    if let Some(backtrace) = e.backtrace() {
      writeln!(stderr, "backtrace: {:?}", backtrace).expect(stderr_fail_msg);
    }

    // Exit with a nonzero exit code
    ::std::process::exit(1);
  }
}

fn run() -> Result<()> {
  let matches = App::new(env!("CARGO_PKG_NAME"))
    .version(crate_version!())
    .arg(
      Arg::with_name("no-check-update")
        .help("Disable auto update")
        .long("no-check-update")
        .takes_value(false)
    )
    .get_matches();

  // if matches.is_present("no-check-update") {
  //   // do stuff
  // };

  league_client::run();

  Ok(())
}

#[cfg(test)]
mod tests {

  #[test]
  /// Test something
  fn test_something() {
    unimplemented!();
  }
}
