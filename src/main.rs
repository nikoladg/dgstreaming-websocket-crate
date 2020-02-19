extern crate futures;
extern crate tokio;
extern crate websocket;

use futures::future::Future;
use futures::sink::Sink;
use futures::stream::Stream;
use futures::sync::mpsc;
use hyper::header::{Authorization, Basic, Headers};
use std::fs::File;
use std::io::Read;
use std::mem;
use std::thread;
use websocket::result::WebSocketError;
use websocket::{ClientBuilder, OwnedMessage};

const ENDPOINT: &str = "wss://brain.deepgram.com/v2/listen/stream";

fn main() {
    // the runtime
    let mut runtime = tokio::runtime::current_thread::Builder::new()
        .build()
        .unwrap();

    // headers for authentication
    let mut headers = Headers::new();
    headers.set(Authorization(Basic {
        username: "your_username".to_owned(),
        password: Some("your_password".to_owned()),
    }));

    // a channel for streaming audio data
    let (sender, receiver) = mpsc::channel(0);

    // a thread for reading in and streaming audio data
    thread::spawn(|| {
        let mut sink = sender.wait();
        let mut file = File::open("/path/to/audio/file").unwrap();

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).expect("Error reading audio file.");

        // stream the audio file in by block
        let block_len = 1000;
        while block_len < buffer.len() {
            let mut block = buffer.split_off(block_len);
            mem::swap(&mut block, &mut buffer);
            let msg = OwnedMessage::Binary(block);
            sink.send(msg).expect("Error sending data.");
        }

        let block = buffer.split_off(0);
        let msg = OwnedMessage::Binary(block);
        sink.send(msg).expect("Error sending data.");

        // send an empty message to gracefully close the connection after the response has been received
        sink.send(OwnedMessage::Binary(Vec::new()))
            .expect("Error sending data.");
    });

    // set up the runner to be executed by the runtime
    let runner = ClientBuilder::new(ENDPOINT)
        .unwrap()
        .custom_headers(&headers)
        .async_connect_secure(None)
        .and_then(|(duplex, _)| {
            let (sink, stream) = duplex.split();
            stream
                .filter_map(|response| {
                    // the response from deepgram
                    println!("{:?}", response);
                    match response {
                        OwnedMessage::Close(e) => Some(OwnedMessage::Close(e)),
                        OwnedMessage::Ping(d) => Some(OwnedMessage::Pong(d)),
                        _ => None,
                    }
                })
                .select(receiver.map_err(|_| WebSocketError::NoDataAvailable))
                // forward the audio stream to deepgram
                .forward(sink)
        });

    // execute the runner
    let _ = runtime.block_on(runner).unwrap();
}
