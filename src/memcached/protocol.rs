use tokio_util::codec::Decoder;


use crate::MemcachedError;
// <command_name> <key> <flags> <exptime> <data_len>\r\n
// <data_block>\r\n
// command_name set add replace
#[derive(Debug)]
struct ProtocolCodec {

}
#[allow(dead_code)]
enum Command {
    Get,
    Set,
}

impl Decoder for ProtocolCodec {
    type Item = Command;

    type Error = MemcachedError;

    fn decode(&mut self, _src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        todo!()
    }
}