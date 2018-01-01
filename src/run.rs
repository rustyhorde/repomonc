// Copyright (c) 2017 repomonc developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! `repomonc` runtime
use clap::App;
use error::Result;
use std::io::{self, Write};

/// CLI Runtime
#[allow(dead_code)]
pub fn run() -> Result<i32> {
    let _matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("Prints 'Hello, Rustaceans!' to stdout")
        .get_matches();
    writeln!(io::stdout(), "Hello, Rustaceans!")?;
    Ok(0)
}
