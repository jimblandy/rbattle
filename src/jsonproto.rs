/// A Tokio protocol that sends Rust types serialized as JSON back and forth.

use bytes::BytesMut;
use serde::de::DeserializeOwned;
use serde::ser::Serialize;
use serde_json;
use tokio_core::net::TcpStream;
use tokio_codec::{Decoder, Encoder, Framed};
use tokio_proto::pipeline::ServerProto;

use std::io::{Error, ErrorKind};
use std::marker::PhantomData;

/// A codec that encodes values of type `Out` as JSON, and decodes values of
/// type `In` from JSON.
///
/// JSON values are delimited by newline characters, for simplicity. serde_json
/// never includes unescaped newlines in the JSON itself, except in
/// pretty-printing mode, which we won't use.
pub struct JsonCodec<In, Out> {
    marker: PhantomData<(In, Out)>
}

impl<In, Out> Default for JsonCodec<In, Out> {
    fn default() -> Self { JsonCodec { marker: PhantomData::default() } }
}

impl<In, Out> Decoder for JsonCodec<In, Out>
    where In: DeserializeOwned
{
    type Item = In;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<In>, Error> {
        if let Some(i) = src.iter().position(|b| *b == b'\n') {
            let line = src.split_to(i + 1);
            ::std::str::from_utf8(&line)
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))
                .and_then(|s| {
                    serde_json::from_str(s)
                        .map_err(|e| Error::new(ErrorKind::Other, e))
                })
                .map(Some)
        } else {
            Ok(None)
        }
    }
}

impl<In, Out> Encoder for JsonCodec<In, Out>
    where Out: Serialize
{
    type Item = Out;
    type Error = Error;
    fn encode(&mut self, item: Out, dst: &mut BytesMut) -> Result<(), Error> {
        let mut json = serde_json::to_string(&item)
            .map_err(|e| Error::new(ErrorKind::Other, e))?;
        json.push('\n');
        dst.extend(json.as_bytes());
        Ok(())
    }
}

/// A Tokio protocol that receives values of type `In` and transmits values of
/// type `Out`, serialized as JSON.
pub struct JsonProto<In, Out> {
    marker: PhantomData<(In, Out)>
}

impl<In, Out> JsonProto<In, Out> {
    pub fn new() -> Self {
        JsonProto { marker: PhantomData::default() }
    }
}

impl<In, Out> ServerProto<TcpStream> for JsonProto<In, Out>
    where In: 'static + DeserializeOwned,
          Out: 'static + Serialize
{
    type Request = In;
    type Response = Out;
    type Transport = Framed<TcpStream, JsonCodec<In, Out>>;
    type BindTransport = Result<Self::Transport, Error>;
    fn bind_transport(&self, io: TcpStream) -> Self::BindTransport {
        io.set_nodelay(true)?;
        Ok(JsonCodec::default().framed(io))
    }
}
