use crate::update_protocol::{self as protocol, Action, Phase, Status};
use std::{
    net::{SocketAddr, TcpStream},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

pub struct Client {
    state: Arc<Mutex<Status>>,
    commands: Option<mpsc::SyncSender<Action>>,
}
impl Client {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(Status::default()));
        let endpoint = std::env::var(protocol::ENDPOINT_ENV)
            .ok()
            .and_then(|v| v.parse::<SocketAddr>().ok())
            .filter(|v| v.ip().is_loopback());
        let token = std::env::var(protocol::TOKEN_ENV)
            .ok()
            .filter(|v| v.len() == 64 && v.bytes().all(|c| c.is_ascii_hexdigit()));
        let (Some(endpoint), Some(token)) = (endpoint, token) else {
            return Self {
                state,
                commands: None,
            };
        };
        let (commands, receive) = mpsc::sync_channel(4);
        let copy = state.clone();
        std::thread::spawn(move || {
            let mut action = Action::Poll;
            loop {
                let result = (|| -> std::io::Result<Status> {
                    let mut stream =
                        TcpStream::connect_timeout(&endpoint, Duration::from_millis(400))?;
                    stream.set_read_timeout(Some(Duration::from_millis(700)))?;
                    stream.set_write_timeout(Some(Duration::from_millis(700)))?;
                    protocol::write(
                        &mut stream,
                        &protocol::Request {
                            token: token.clone(),
                            action,
                        },
                    )?;
                    protocol::read(&mut stream)
                })();
                match result {
                    Ok(mut status) => {
                        status.notes = protocol::notes(&status.notes);
                        *copy.lock().unwrap() = status;
                    }
                    Err(_) => {
                        *copy.lock().unwrap() = Status {
                            phase: Phase::Error,
                            message: "The update service is unavailable. Try again.".into(),
                            ..Default::default()
                        }
                    }
                }
                action = match receive.recv_timeout(Duration::from_secs(1)) {
                    Ok(action) => action,
                    Err(mpsc::RecvTimeoutError::Timeout) => Action::Poll,
                    Err(_) => return,
                };
            }
        });
        Self {
            state,
            commands: Some(commands),
        }
    }
    pub fn snapshot(&self) -> Status {
        self.state.lock().unwrap().clone()
    }
    pub fn send(&self, action: Action) {
        if let Some(commands) = &self.commands {
            let _ = commands.try_send(action);
        }
    }
    #[cfg(test)]
    pub fn fixture(status: Status) -> (Self, mpsc::Receiver<Action>) {
        let (send, receive) = mpsc::sync_channel(4);
        (
            Self {
                state: Arc::new(Mutex::new(status)),
                commands: Some(send),
            },
            receive,
        )
    }
}
