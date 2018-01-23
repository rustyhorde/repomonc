// Copyright (c) 2017 repomonc developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! An example of hooking up stdin/stdout to either a TCP or UDP stream.
//!
//! This example will connect to a socket address specified in the argument list
//! and then forward all data read on stdin to the server, printing out all data
//! received on stdout. An optional `--udp` argument can be passed to specify
//! that the connection should be made over UDP instead of TCP, translating each
//! line entered on stdin to a UDP packet to be sent to the remote address.
//!
//! Note that this is not currently optimized for performance, especially
//! around buffer management. Rather it's intended to show an example of
//! working with a client.
//!
//! This example can be quite useful when interacting with the other examples in
//! this repository! Many of them recommend running this as a simple "hook up
//! stdin/stdout to a server" to get up and running.
#![deny(missing_docs)]

#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate getset;
#[macro_use]
extern crate slog;
#[macro_use]
extern crate slog_try;

extern crate bincode;
extern crate bytes;
extern crate clap;
extern crate futures;
extern crate repomon;
extern crate slog_async;
extern crate slog_term;
extern crate tokio_core;
extern crate tokio_io;

use std::io::{self, Write};
use std::process;

mod error;
mod log;
mod run;
mod tcp;
mod udp;

/// CLI Entry Point
fn main() {
    match run::run() {
        Ok(i) => process::exit(i),
        Err(e) => {
            writeln!(io::stderr(), "{}", e).expect("Unable to write to stderr!");
            process::exit(1)
        }
    }
}
