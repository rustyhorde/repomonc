// Copyright (c) 2017 repomonc developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! TCP future stream handling.
use std::io;
use std::net::SocketAddr;

use bincode::{deserialize, serialize, Infinite};
use bytes::BytesMut;
use futures::{Future, Stream};
use repomon::Message;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use tokio_io::AsyncRead;
use tokio_io::codec::{Decoder, Encoder};

/// Connect to the given address via a `TcpStream`.
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
        if buf.is_empty() {
            Ok(None)
        } else {
            let len = buf.len();
            let bytes = buf.split_to(len);

            match deserialize(bytes.as_ref()) {
                Ok(message) => Ok(Some(message)),
                Err(e) => {
                    writeln!(io::stderr(), "{}", e)?;
                    Ok(None)
                }
            }
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
