//! Ordered codec policy shared by room negotiation and decode recovery.
use crate::quality::Codec;

pub const ORDER: [Codec; 3] = [Codec::Av1, Codec::H265, Codec::H264];
pub const CAPABILITIES: &str = "freedomstream.decode.v1";
pub const DECODE_PREFIX: &str = "freedomstream.decode.publisher.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Codecs(u8);
impl Codecs {
    pub fn new(av1: bool, hevc: bool, h264: bool) -> Self {
        Self(u8::from(av1) | (u8::from(hevc) << 1) | (u8::from(h264) << 2))
    }
    pub fn supports(self, codec: Codec) -> bool {
        self.0 & bit(codec) != 0
    }
    pub fn reject(&mut self, codec: Codec) {
        self.0 &= !bit(codec);
    }
    pub fn insert(&mut self, codec: Codec) {
        self.0 |= bit(codec);
    }
    pub fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
    pub fn encode(self) -> String {
        ORDER
            .into_iter()
            .filter(|c| self.supports(*c))
            .map(name)
            .collect::<Vec<_>>()
            .join(",")
    }
    pub fn parse(value: &str) -> Self {
        // Unknown tokens never authorize an unadvertised decoder.
        let mut result = Self::default();
        for codec in ORDER {
            if value.split(',').any(|token| token == name(codec)) {
                result.0 |= bit(codec);
            }
        }
        result
    }
    pub fn best(self, preferred: Codec, can_encode: impl Fn(Codec) -> bool) -> Option<Codec> {
        ORDER
            .into_iter()
            .skip_while(|codec| *codec != preferred)
            .find(|codec| self.supports(*codec) && can_encode(*codec))
    }
}
fn bit(codec: Codec) -> u8 {
    match codec {
        Codec::Av1 => 1,
        Codec::H265 => 2,
        Codec::H264 => 4,
    }
}
pub fn name(codec: Codec) -> &'static str {
    match codec {
        Codec::Av1 => "av1",
        Codec::H265 => "hevc",
        Codec::H264 => "h264",
    }
}
pub fn from_mime(mime: &str) -> Option<Codec> {
    ORDER.into_iter().find(|codec| {
        mime.eq_ignore_ascii_case(match codec {
            Codec::Av1 => "video/av1",
            Codec::H265 => "video/h265",
            Codec::H264 => "video/h264",
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranked_intersection_handles_gaps_membership_and_explicit_preference() {
        let all = Codecs::new(true, true, true);
        let hevc = Codecs::new(false, true, true);
        let h264 = Codecs::new(false, false, true);
        assert_eq!(all.best(Codec::Av1, |_| true), Some(Codec::Av1));
        assert_eq!(
            all.intersect(hevc).best(Codec::Av1, |_| true),
            Some(Codec::H265)
        );
        assert_eq!(
            all.intersect(hevc)
                .intersect(h264)
                .best(Codec::Av1, |_| true),
            Some(Codec::H264)
        );
        assert_eq!(
            all.intersect(hevc).best(Codec::Av1, |c| c != Codec::H265),
            Some(Codec::H264)
        );
        assert_eq!(all.best(Codec::H265, |_| true), Some(Codec::H265));
        assert_eq!(all.best(Codec::H264, |_| true), Some(Codec::H264));
        assert_eq!(
            Codecs::new(true, false, false)
                .intersect(hevc)
                .best(Codec::Av1, |_| true),
            None
        );
    }
    #[test]
    fn failures_remove_only_the_failed_codec_and_do_not_reset_on_republication() {
        let mut decoders = Codecs::new(true, true, true);
        decoders.reject(Codec::Av1);
        assert_eq!(decoders.best(Codec::Av1, |_| true), Some(Codec::H265));
        decoders.reject(Codec::H265);
        assert_eq!(decoders.best(Codec::Av1, |_| true), Some(Codec::H264));
        assert_eq!(Codecs::parse(&decoders.encode()), decoders);
        assert_eq!(
            Codecs::parse("future,AV1,h264"),
            Codecs::new(false, false, true)
        );
        assert_eq!(Codecs::parse(""), Codecs::default());
    }
}
