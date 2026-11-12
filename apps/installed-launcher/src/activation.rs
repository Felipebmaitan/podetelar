use crate::{activation_protocol as protocol, registration};
use std::{
    net::{Ipv4Addr, TcpListener},
    sync::mpsc,
    time::Duration,
};
pub struct Instance {
    _lock: std::fs::File,
    pub address: String,
    pub token: String,
}
impl Drop for Instance {
    fn drop(&mut self) {
        registration::clear();
    }
}
pub fn forward(link: Option<String>) -> anyhow::Result<()> {
    for _ in 0..10 {
        if let Ok((address, token)) = registration::active()
            && protocol::exchange(&address, &token, false, link.clone()).is_ok_and(|v| v.accepted)
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    anyhow::bail!("PodeTelar is busy. Try opening the room link again.")
}
impl Instance {
    pub fn start() -> std::io::Result<Option<Self>> {
        let exe = std::env::current_exe()?;
        let root = exe
            .parent()
            .and_then(std::path::Path::parent)
            .ok_or(std::io::ErrorKind::NotFound)?;
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let lock = match options.open(root.join("installed-launcher.lock")) {
            Ok(lock) => lock,
            Err(error)
                if error.kind() == std::io::ErrorKind::PermissionDenied
                    || error.raw_os_error() == Some(32) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?.to_string();
        let token = rand::random::<[u8; 32]>()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        registration::publish(&address, &token)
            .map_err(|_| std::io::ErrorKind::PermissionDenied)?;
        let expected = token.clone();
        let (send, receive) = mpsc::sync_channel::<Option<String>>(1);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else {
                    break;
                };
                let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
                let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
                let Ok(request) = protocol::read::<protocol::Request>(&mut stream) else {
                    continue;
                };
                if request.token != expected
                    || request.link.as_ref().is_some_and(|v| !protocol::valid(v))
                {
                    continue;
                }
                let reply = if request.poll {
                    match receive.try_recv() {
                        Ok(link) => protocol::Reply {
                            accepted: true,
                            focus: true,
                            link,
                        },
                        Err(_) => protocol::Reply {
                            accepted: true,
                            ..Default::default()
                        },
                    }
                } else {
                    protocol::Reply {
                        accepted: send.try_send(request.link).is_ok(),
                        ..Default::default()
                    }
                };
                let _ = protocol::write(&mut stream, &reply);
            }
        });
        Ok(Some(Self {
            _lock: lock,
            address,
            token,
        }))
    }
}
