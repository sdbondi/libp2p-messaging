use crate::codec::Codec;
use std::fmt;

pub struct Message<TCodec: Codec> {
    pub(crate) message: TCodec::Message,
    pub(crate) protocol: TCodec::Protocol,
}

impl<TCodec> fmt::Debug for Message<TCodec>
where
    TCodec: Codec,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Message").finish_non_exhaustive()
    }
}
