// Copyright (c) 2017 repomonc developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! UDP future stream handling.
use std::io;
use std::net::SocketAddr;

use bincode::{deserialize, serialize, Infinite};
use futures::{Future, Stream};
use repomon::Message;
use tokio_core::net::{UdpCodec, UdpSocket};
use tokio_core::reactor::Handle;

/// Connect the the given address via a `UdpSocket`.
pub fn connect(
    &addr: &SocketAddr,
    handle: &Handle,
    stdin: Box<dyn Stream<Item = Message, Error = io::Error>>,
) -> Box<dyn Stream<Item = Message, Error = io::Error>> {
    // We'll bind our UDP socket to a local IP/port, but for now we
    // basically let the OS pick both of those.
    let addr_to_bind = if addr.ip().is_ipv4() {
        "0.0.0.0:0".parse().expect("failed to parse ipv4 address")
    } else {
        "[::]:0".parse().expect("failed to parse ipv6 address")
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

/// Bytes Unit Struct
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
