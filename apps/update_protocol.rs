//! Private, per-launch loopback transport. Never contains room/media authority.
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

pub const LIMIT: usize = 16 * 1024;
pub const ENDPOINT_ENV: &str = "FREEDOMSTREAM_UPDATE_ENDPOINT";
pub const TOKEN_ENV: &str = "FREEDOMSTREAM_UPDATE_TOKEN";

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Disabled,
    Checking,
    Current,
    Available,
    Downloading,
    Ready,
    Restarting,
    Error,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub phase: Phase,
    pub version: String,
    pub notes: String,
    pub progress: u8,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Action {
    Poll,
    Check,
    Download,
    Restart,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub token: String,
    pub action: Action,
}

pub fn read<T: serde::de::DeserializeOwned>(stream: &mut impl Read) -> std::io::Result<T> {
    let mut size = [0; 4];
    stream.read_exact(&mut size)?;
    let size = u32::from_le_bytes(size) as usize;
    if size > LIMIT {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    let mut bytes = vec![0; size];
    stream.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(|_| std::io::ErrorKind::InvalidData.into())
}

pub fn write<T: Serialize>(stream: &mut impl Write, value: &T) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > LIMIT {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    stream.write_all(&(bytes.len() as u32).to_le_bytes())?;
    stream.write_all(&bytes)
}

/// Bounded plain text; preserve newlines, omit controls and never render HTML.
pub fn notes(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(1200)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oversized_and_truncated_messages_fail_closed() {
        assert!(read::<Request>(&mut &(u32::MAX.to_le_bytes())[..]).is_err());
        assert!(read::<Request>(&mut &[10, 0, 0, 0, b'{'][..]).is_err());
        let mut bytes = Vec::new();
        write(
            &mut bytes,
            &Request {
                token: "local-test".into(),
                action: Action::Download,
            },
        )
        .unwrap();
        assert_eq!(
            read::<Request>(&mut &bytes[..]).unwrap().action,
            Action::Download
        );
        assert_eq!(notes(&"x".repeat(5000)).len(), 1200);
        assert_eq!(notes("a\0\r\nb"), "a\nb");
    }
}
