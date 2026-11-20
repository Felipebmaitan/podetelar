//! Hardware compatibility is independent of playback health.
use crate::quality::Codec;

pub const ORDER: [Codec; 3] = [Codec::Av1, Codec::H265, Codec::H264];
pub const CAPABILITIES: &str = "podetelar.decode.v1";
pub const HARDWARE_CAPABILITIES: &str = "podetelar.decode.hardware.v1";
pub const DECODE_PREFIX: &str = "podetelar.decode.publisher.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Codecs(u8);
impl Codecs {
    /// The pinned frame cryptor encrypts every AV1 OBU byte, but the AV1 RTP
    /// packetizer needs those headers. H.26x retains its packetization headers.
    pub fn for_room(protected: bool) -> Self {
        Self::new(!protected, true, true)
    }

    pub fn new(av1: bool, hevc: bool, h264: bool) -> Self {
        Self(u8::from(av1) | (u8::from(hevc) << 1) | (u8::from(h264) << 2))
    }
    pub fn supports(self, codec: Codec) -> bool {
        self.0 & bit(codec) != 0
    }
    pub fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
    pub fn encode(self) -> String {
        let names = self.names();
        // Empty attribute values delete keys in LiveKit. An explicitly empty
        // hardware set must remain distinguishable from a pending announcement.
        if names.is_empty() {
            "none".into()
        } else {
            names.join(",")
        }
    }
    pub fn names(self) -> Vec<&'static str> {
        ORDER
            .into_iter()
            .filter(|c| self.supports(*c))
            .map(name)
            .collect()
    }
    pub fn parse(value: &str) -> Option<Self> {
        if value == "none" {
            return Some(Self::default());
        }
        let mut result = Self::default();
        for token in value.split(',') {
            let codec = ORDER.into_iter().find(|codec| token == name(*codec))?;
            if result.supports(codec) {
                return None;
            }
            result.0 |= bit(codec);
        }
        Some(result)
    }
    pub fn common(previous: Option<Self>, member: Option<Self>) -> Option<Self> {
        Some(previous?.intersect(member?))
    }
    pub fn select(
        self,
        preferred: Codec,
        current: Option<Codec>,
        can_encode: impl Fn(Codec) -> bool,
    ) -> Option<Codec> {
        // A compatible live codec stays stable, even after restrictions leave.
        current
            .filter(|codec| self.supports(*codec))
            .or_else(|| self.best(preferred, can_encode))
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
    fn live_codec_changes_only_when_explicit_capabilities_exclude_it() {
        let all = Codecs::new(true, true, true);
        let hevc = Codecs::new(false, true, true);
        let h264 = Codecs::new(false, false, true);
        let mut current = all.select(Codec::Av1, None, |_| true);
        assert_eq!(current, Some(Codec::Av1));
        current = hevc.select(Codec::Av1, current, |_| true);
        assert_eq!(current, Some(Codec::H265));
        current = h264.select(Codec::Av1, current, |_| true);
        assert_eq!(current, Some(Codec::H264));
        assert_eq!(all.select(Codec::Av1, current, |_| true), current);
        assert_eq!(all.select(Codec::Av1, None, |_| true), Some(Codec::Av1));
    }
    #[test]
    fn missing_malformed_and_explicitly_empty_capabilities_are_distinct() {
        let all = Codecs::new(true, true, true);
        assert_eq!(Codecs::parse(&all.encode()), Some(all));
        assert_eq!(
            Codecs::parse(&Codecs::default().encode()),
            Some(Codecs::default())
        );
        for malformed in ["", "future,AV1,h264", "av1,av1", "av1,"] {
            assert_eq!(Codecs::parse(malformed), None);
        }
        assert_eq!(Codecs::common(Some(all), None), None);
        assert_eq!(
            Codecs::common(Some(all), Some(Codecs::default())),
            Some(Codecs::default())
        );
    }
}
