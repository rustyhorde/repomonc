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

extern crate bincode;
extern crate bytes;
extern crate clap;
extern crate futures;
extern crate repomon_config;
extern crate tokio_core;
extern crate tokio_io;

use std::env;
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::thread;

use futures::sync::mpsc;
use futures::{Future, Sink, Stream};
use repomon_config::Message;
use tokio_core::reactor::Core;

mod error;
mod run;

// use std::io::{self, Write};
// use std::process;

/// CLI Entry Point
// fn main() {
//     match run::run() {
//         Ok(i) => process::exit(i),
//         Err(e) => {
//             writeln!(io::stderr(), "{}", e).expect("Unable to write to stderr!");
//             process::exit(1)
//         }
//     }
// }

fn main() {
    // Determine if we're going to run in TCP or UDP mode
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let tcp = match args.iter().position(|a| a == "--udp") {
        Some(i) => {
            args.remove(i);
            false
        }
        None => true,
    };

    // Parse what address we're going to connect to
    let addr = args.first()
        .unwrap_or_else(|| panic!("this program requires at least one argument"));
    let addr = addr.parse::<SocketAddr>().unwrap();

    // Create the event loop and initiate the connection to the remote server
    let mut core = Core::new().unwrap();
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
    let stdout = if tcp {
        tcp::connect(&addr, &handle, Box::new(stdin_rx))
    } else {
        udp::connect(&addr, &handle, Box::new(stdin_rx))
    };

    // And now with our stream of bytes to write to stdout, we execute that in
    // the event loop! Note that this is doing blocking I/O to emit data to
    // stdout, and in general it's a no-no to do that sort of work on the event
    // loop. In this case, though, we know it's ok as the event loop isn't
    // otherwise running anything useful.
    let mut out = io::stdout();
    core.run(stdout.for_each(|chunk| {
        out.write_all("New Message\n".as_bytes()).expect("");
        out.write_all(format!("{}\n", &chunk).as_bytes()).expect("");
        out.flush().expect("");
        Ok(())
    })).unwrap();
}

mod tcp {
    use std::io;
    use std::net::SocketAddr;

    use bincode::{deserialize, serialize, Infinite};
    use bytes::BytesMut;
    use futures::{Future, Stream};
    use repomon_config::Message;
    use tokio_core::net::TcpStream;
    use tokio_core::reactor::Handle;
    use tokio_io::AsyncRead;
    use tokio_io::codec::{Decoder, Encoder};

    pub fn connect(
        addr: &SocketAddr,
        handle: &Handle,
        stdin: Box<Stream<Item = Message, Error = io::Error>>,
    ) -> Box<Stream<Item = Message, Error = io::Error>> {
        let tcp = TcpStream::connect(addr, handle);
        let handle = handle.clone();

        // After the TCP connection has been established, we set up our client
        // to start forwarding data.
        //
        // First we use the `Io::framed` method with a simple implementation of
        // a `Codec` (listed below) that just ships bytes around. We then split
        // that in two to work with the stream and sink separately.
        //
        // Half of the work we're going to do is to take all data we receive on
        // `stdin` and send that along the TCP stream (`sink`). The second half
        // is to take all the data we receive (`stream`) and then write that to
        // stdout. We'll be passing this handle back out from this method.
        //
        // You'll also note that we *spawn* the work to read stdin and write it
        // to the TCP stream. This is done to ensure that happens concurrently
        // with us reading data from the stream.
        Box::new(tcp.map(move |stream| {
            let (sink, stream) = stream.framed(Bytes).split();
            handle.spawn(stdin.forward(sink).then(|result| {
                if let Err(e) = result {
                    panic!("failed to write to socket: {}", e)
                }
                Ok(())
            }));
            stream
        }).flatten_stream())
    }

    /// A simple `Codec` implementation that just ships bytes around.
    ///
    /// This type is used for "framing" a TCP stream of bytes but it's really
    /// just a convenient method for us to work with streams/sinks for now.
    /// This'll just take any data read and interpret it as a "frame" and
    /// conversely just shove data into the output location without looking at
    /// it.
    struct Bytes;

    impl Decoder for Bytes {
        type Item = Message;
        type Error = io::Error;

        fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<Message>> {
            use std::io::{self, Write};
            if buf.len() > 0 {
                let len = buf.len();
                let bytes = buf.split_to(len);

                match deserialize(bytes.as_ref()) {
                    Ok(message) => Ok(Some(message)),
                    Err(e) => {
                        writeln!(io::stderr(), "{}", e)?;
                        Ok(None)
                    }
                }
            } else {
                Ok(None)
            }
        }

        // fn decode_eof(&mut self, buf: &mut BytesMut) -> io::Result<Option<Message>> {
        //     self.decode(buf)
        // }
    }

    impl Encoder for Bytes {
        type Item = Message;
        type Error = io::Error;

        fn encode(&mut self, data: Message, buf: &mut BytesMut) -> io::Result<()> {
            match serialize(&data, Infinite) {
                Ok(bytes) => {
                    buf.extend(bytes.iter());
                    Ok(())
                }
                Err(_e) => Ok(()),
            }
        }
    }
}

mod udp {
    use std::io;
    use std::net::SocketAddr;

    use bincode::{deserialize, serialize, Infinite};
    use futures::{Future, Stream};
    use repomon_config::Message;
    use tokio_core::net::{UdpCodec, UdpSocket};
    use tokio_core::reactor::Handle;

    pub fn connect(
        &addr: &SocketAddr,
        handle: &Handle,
        stdin: Box<Stream<Item = Message, Error = io::Error>>,
    ) -> Box<Stream<Item = Message, Error = io::Error>> {
        // We'll bind our UDP socket to a local IP/port, but for now we
        // basically let the OS pick both of those.
        let addr_to_bind = if addr.ip().is_ipv4() {
            "0.0.0.0:0".parse().unwrap()
        } else {
            "[::]:0".parse().unwrap()
        };
        let udp = UdpSocket::bind(&addr_to_bind, handle).expect("failed to bind socket");

        // Like above with TCP we use an instance of `UdpCodec` to transform
        // this UDP socket into a framed sink/stream which operates over
        // discrete values. In this case we're working with *pairs* of socket
        // addresses and byte buffers.
        let (sink, stream) = udp.framed(Bytes).split();

        // All bytes from `stdin` will go to the `addr` specified in our
        // argument list. Like with TCP this is spawned concurrently
        handle.spawn(
            stdin
                .map(move |chunk| (addr, chunk))
                .forward(sink)
                .then(|result| {
                    if let Err(e) = result {
                        panic!("failed to write to socket: {}", e)
                    }
                    Ok(())
                }),
        );

        // With UDP we could receive data from any source, so filter out
        // anything coming from a different address
        Box::new(stream.filter_map(
            move |(src, chunk)| {
                if src == addr {
                    Some(chunk)
                } else {
                    None
                }
            },
        ))
    }

    struct Bytes;

    impl UdpCodec for Bytes {
        type In = (SocketAddr, Message);
        type Out = (SocketAddr, Message);

        fn decode(&mut self, addr: &SocketAddr, buf: &[u8]) -> io::Result<Self::In> {
            match deserialize(buf) {
                Ok(message) => Ok((*addr, message)),
                Err(_) => Ok((*addr, Default::default())),
            }
        }

        fn encode(&mut self, (addr, message): Self::Out, into: &mut Vec<u8>) -> SocketAddr {
            match serialize(&message, Infinite) {
                Ok(bytes) => {
                    into.extend(bytes.iter());
                }
                Err(_e) => {}
            }
            addr
        }
    }
}

// Our helper method which will read data from stdin and send it along the
// sender provided.
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
