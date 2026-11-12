//! Per-launch local activation. Tickets live only in memory.
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
pub const ENDPOINT: &str = "FREEDOMSTREAM_ACTIVATION_ENDPOINT";
pub const TOKEN: &str = "FREEDOMSTREAM_ACTIVATION_TOKEN";
pub const INSTALLED: &str = "FREEDOMSTREAM_INSTALLED_EXE";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub token: String,
    pub poll: bool,
    pub link: Option<String>,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub accepted: bool,
    pub focus: bool,
    pub link: Option<String>,
}
pub fn valid(link: &str) -> bool {
    link.len() <= 4096
        && link.starts_with("freedomstream://join")
        && !link.chars().any(|c| c.is_control() || c == '"')
}
pub fn read<T: serde::de::DeserializeOwned>(stream: &mut impl Read) -> std::io::Result<T> {
    let mut size = [0; 4];
    stream.read_exact(&mut size)?;
    let size = u32::from_le_bytes(size) as usize;
    if size > 8192 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    let mut bytes = vec![0; size];
    stream.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(|_| std::io::ErrorKind::InvalidData.into())
}
pub fn write<T: Serialize>(stream: &mut impl Write, value: &T) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > 8192 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    stream.write_all(&(bytes.len() as u32).to_le_bytes())?;
    stream.write_all(&bytes)
}
pub fn exchange(
    address: &str,
    token: &str,
    poll: bool,
    link: Option<String>,
) -> std::io::Result<Reply> {
    use std::{
        net::{SocketAddr, TcpStream},
        time::Duration,
    };
    let address: SocketAddr = address
        .parse()
        .map_err(|_| std::io::ErrorKind::InvalidInput)?;
    if !address.ip().is_loopback() {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(500))?;
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    write(
        &mut stream,
        &Request {
            token: token.into(),
            poll,
            link,
        },
    )?;
    read(&mut stream)
}
