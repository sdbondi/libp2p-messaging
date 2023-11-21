use libp2p_messaging::codecs::prost::ProstCodec;

#[derive(prost::Message, )]
pub struct TestMessage {}

#[test]
fn todo() {
    let mut codec = ProstCodec::<TestMessage>::default();
}
