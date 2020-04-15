//! An interactive serial terminal

use bytes::BufMut;
use bytes::BytesMut;
use futures::stream::StreamExt;
use std::{env, io, str};
use structopt::StructOpt;
use tokio_util::codec::{Decoder, Encoder};
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec, LinesCodecError};

const DEFAULT_TTY: &str = "/dev/ttyACM0";

#[derive(Debug, StructOpt)]
struct Opt {}

struct SerialReadCodec;

impl Decoder for SerialReadCodec {
    type Item = String;
    type Error = LinesCodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let newline = src.as_ref().iter().position(|b| *b == b'\n');
        if let Some(n) = newline {
            let line = src.split_to(n + 1);
            let line = &line[..line.len() - 2];
            return match str::from_utf8(line.as_ref()) {
                Ok(s) => Ok(Some(s.to_string())),
                Err(_) => Err(LinesCodecError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    "Invalid String",
                ))),
            };
        }

        Ok(None)
    }
}

struct SerialWriteCodec;

impl Encoder<String> for SerialWriteCodec {
    type Error = LinesCodecError;

    fn encode(&mut self, line: String, buf: &mut BytesMut) -> Result<(), Self::Error> {
        buf.reserve(line.len());
        buf.put(line.as_bytes());
        buf.put_u8(b'\r');
        buf.put_u8(b'\n');

        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let _ = Opt::from_args();

    let mut args = env::args();
    let tty_path = args.nth(1).unwrap_or_else(|| DEFAULT_TTY.into());

    let mut settings = tokio_serial::SerialPortSettings::default();
    settings.baud_rate = 921600;
    settings.data_bits = tokio_serial::DataBits::Eight;
    settings.flow_control = tokio_serial::FlowControl::None;
    settings.parity = tokio_serial::Parity::None;
    settings.stop_bits = tokio_serial::StopBits::One;
    settings.timeout = std::time::Duration::from_secs(5);

    let mut serial = tokio_serial::Serial::from_path(tty_path, &settings).unwrap();

    serial
        .set_exclusive(false)
        .expect("Unable to set serial port exclusive to false");

    let stdout = tokio::io::stdout();
    let stdin = tokio::io::stdin();
    let framed_stdin = FramedRead::new(stdin, LinesCodec::new());
    let framed_stdout = FramedWrite::new(stdout, LinesCodec::new());

    let (read, write) = tokio::io::split(serial);
    let stream = FramedRead::new(read, SerialReadCodec);
    let sink = FramedWrite::new(write, SerialWriteCodec);

    let input = framed_stdin.forward(sink);
    let output = stream.forward(framed_stdout);
    let result = futures::future::try_join(input, output).await;

    println!("Uh oh: {:?}", result);
}