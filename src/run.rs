// Copyright (c) 2017 repomonc developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! `repomonc` runtime
use clap::{App, Arg};
use error::Result;
use futures::sync::mpsc;
use futures::{Future, Sink, Stream};
use repomon::Message;
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::thread;
use tcp;
use tokio_core::reactor::Core;
use udp;

/// CLI Runtime
#[allow(dead_code)]
pub fn run() -> Result<i32> {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("Connects to a repomons server to receive notifications")
        .arg(Arg::with_name("udp").short("u").long("udp"))
        .arg(Arg::with_name("address").default_value("127.0.0.1:8080"))
        .get_matches();

    // Parse what address we're going to connect to
    let addr = matches
        .value_of("address")
        .ok_or("invalid address")?
        .parse::<SocketAddr>()?;

    // Create the event loop and initiate the connection to the remote server
    let mut core = Core::new()?;
    let handle = core.handle();

    // Right now Tokio doesn't support a handle to stdin running on the event
    // loop, so we farm out that work to a separate thread. This thread will
    // read data (with blocking I/O) from stdin and then send it to the event
    // loop over a standard futures channel.
    let (stdin_tx, stdin_rx) = mpsc::channel(0);
    thread::spawn(|| read_stdin(stdin_tx));
    let stdin_rx = stdin_rx.map_err(|_| panic!()); // errors not possible on rx

    // Now that we've got our stdin read we either set up our TCP connection or
    // our UDP connection to get a stream of bytes we're going to emit to
    // stdout.
    let stdout = if matches.is_present("udp") {
        udp::connect(&addr, &handle, Box::new(stdin_rx))
    } else {
        tcp::connect(&addr, &handle, Box::new(stdin_rx))
    };

    // And now with our stream of bytes to write to stdout, we execute that in
    // the event loop! Note that this is doing blocking I/O to emit data to
    // stdout, and in general it's a no-no to do that sort of work on the event
    // loop. In this case, though, we know it's ok as the event loop isn't
    // otherwise running anything useful.
    let mut out = io::stdout();
    core.run(stdout.for_each(|chunk| {
        out.write_all(b"New Message\n").expect("");
        out.write_all(format!("{}\n", &chunk).as_bytes()).expect("");
        out.flush().expect("");
        Ok(())
    }))?;

    Ok(0)
}

/// Our helper method which will read data from stdin and send it along the
/// sender provided.
fn read_stdin(mut tx: mpsc::Sender<Message>) {
    let mut stdin = io::stdin();
    loop {
        let mut buf = vec![0; 1024];
        let n = match stdin.read(&mut buf) {
            Err(_) | Ok(0) => break,
            Ok(n) => n,
        };
        buf.truncate(n);
        tx = match tx.send(Default::default()).wait() {
            Ok(tx) => tx,
            Err(_) => break,
        };
    }
}
