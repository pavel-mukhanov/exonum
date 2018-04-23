use bytes::BytesMut;
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use tokio_io::codec::{Decoder, Encoder};
use snow::Session;

use std::io;
use messages::RawMessage;
use messages::MessageBuffer;

pub struct NoiseCodec {
    session: Session,
}

impl NoiseCodec {
    pub fn new(session: Session) -> Self {
        NoiseCodec { session }
    }
}

impl Decoder for NoiseCodec {
    type Item = RawMessage;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, io::Error> {
        if buf.len() < 2 {
            return Ok(None);
        };

        let len = BigEndian::read_u16(buf) as usize;
        let data = buf.split_to(len + 2).to_vec();
        let data = &data[2..];
        let mut read_to = vec![0u8; len];
        //TODO: read messages bigger than 2^16
        self.session.read_message(data, &mut read_to).unwrap();

        let total_len = LittleEndian::read_u32(&read_to[6..10]) as usize;

        let data = read_to.split_at(total_len);
        let raw = RawMessage::new(MessageBuffer::from_vec(Vec::from(data.0)));
        Ok(Some(raw))
    }
}

impl Encoder for NoiseCodec {
    type Item = RawMessage;
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
        let mut tmp_buf = vec![0u8; 65535];
        let len = self.session
            .write_message(msg.as_ref(), &mut tmp_buf)
            .unwrap();
        let mut msg_len_buf = vec![(len >> 8) as u8, (len & 0xff) as u8];
        let tmp_buf = &tmp_buf[0..len];
        msg_len_buf.extend_from_slice(tmp_buf);
        buf.extend_from_slice(&msg_len_buf);
        Ok(())
    }
}
