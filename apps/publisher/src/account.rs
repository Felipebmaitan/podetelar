//! Browser OAuth completion is polled with a native-held proof; only PodeTelar credentials are saved.
use anyhow::{Context, Result, ensure};
use base64::Engine;
use reqwest::{Client as Http, redirect::Policy};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
pub const DEFAULT_SERVER: &str = "https://stream.maitan.me";
#[derive(Clone)]
#[cfg_attr(
    any(test, debug_assertions, feature = "media-qualification"),
    derive(serde::Deserialize)
)]
#[cfg_attr(
    any(test, debug_assertions, feature = "media-qualification"),
    serde(deny_unknown_fields)
)]
pub struct Credential {
    pub server: String,
    token: String,
}
impl Credential {
    pub fn for_server(&self, server: &str) -> Option<&str> {
        (self.server == server).then_some(self.token.as_str())
    }
    pub fn authorize(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request.bearer_auth(&self.token)
    }
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub session_id: String,
    pub user_id: String,
    pub display_name: String,
    pub expires_at: u64,
}
#[derive(Clone)]
pub struct State {
    pub server: String,
    pub account: Option<Account>,
    pub credential: Option<Credential>,
    pub busy: bool,
    pub error: String,
    pub available: bool,
    attempt_generation: u64,
}
impl Default for State {
    fn default() -> Self {
        Self {
            server: DEFAULT_SERVER.into(),
            account: None,
            credential: None,
            busy: false,
            error: String::new(),
            available: false,
            attempt_generation: 0,
        }
    }
}
enum Command {
    SignIn(String, u64),
    SignOut,
    Cancel,
}
pub struct Client {
    state: Arc<Mutex<State>>,
    commands: Option<mpsc::Sender<Command>>,
    browser: Arc<Mutex<Option<String>>>,
    stop: Arc<AtomicBool>,
}
impl Client {
    pub fn new() -> Self {
        let server = crate::settings::login_server().unwrap_or_else(|| DEFAULT_SERVER.into());
        let state = Arc::new(Mutex::new(State {
            server: server.clone(),
            ..Default::default()
        }));
        let browser = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let shared = state.clone();
        let launch = browser.clone();
        let stopped = stop.clone();
        std::thread::Builder::new().name("podetelar-discord".into()).spawn(move||{
            let Ok(runtime)=tokio::runtime::Builder::new_current_thread().enable_all().build()else{return;};
            runtime.block_on(async move{
                let Ok(http)=Http::builder().redirect(Policy::none()).timeout(Duration::from_secs(8)).build()else{return;};
                let mut server=server;let mut credential=match load(&server){Ok(value)=>value,Err(_)=>{set_error(&shared,"Could not read remembered Discord login from OS credential storage.");None}};let mut pending:Option<(String,String,Instant,u64)>=None;let mut next=Instant::now();
                while !stopped.load(Ordering::Acquire){
                    if let Ok(command)=rx.try_recv(){
                        match command{
                            Command::SignIn(target, generation)=>{
                                if !shared.lock().is_ok_and(|s| s.attempt_generation == generation) { continue; }
                                if !crate::admission::valid_server(&target)||target.len()>2048{set_error(&shared,"Choose a valid HTTPS PodeTelar server.");continue;}
                                if credential.as_ref().is_some_and(|c|c.server!=target){set_error(&shared,"Disconnect the current Discord account before changing servers.");continue;}
                                server=target;pending=None;let _=crate::settings::save_login_server(&server);
                                update(&shared,|s|{s.server=server.clone();s.busy=true;s.error.clear();});
                                let proof=base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rand::random::<[u8;32]>());let challenge=format!("{:x}",Sha256::digest(proof.as_bytes()));
                                let result=async{
                                    let response=http.post(format!("{server}/api/auth/discord/start")).header("Origin",&server).json(&serde_json::json!({"challenge":challenge})).send().await?;
                                    let response=checked(response).await?;
                                    #[derive(Deserialize)]#[serde(rename_all="camelCase")]struct Start{request_id:String,authorization_url:String}
                                    let start:Start=serde_json::from_slice(&response)?;let url=url::Url::parse(&start.authorization_url)?;
                                    ensure!(url.origin().ascii_serialization()==server&&url.path()=="/api/auth/discord/authorize"&&url.username().is_empty()&&url.password().is_none(),"Invalid login URL.");
                                    Ok::<_,anyhow::Error>(start)
                                }.await;
                                update(&shared, |state| {
                                    if state.attempt_generation != generation { return; }
                                    match result {
                                        Ok(start) => {
                                            pending = Some((start.request_id, proof, Instant::now() + Duration::from_secs(300), generation));
                                            if let Ok(mut url) = launch.lock() {
                                                *url = Some(start.authorization_url);
                                            }
                                        }
                                        Err(_) => {
                                            state.error = "Could not start Discord sign-in. Check this server's configuration.".into();
                                            state.busy = false;
                                        }
                                    }
                                });
                                next=Instant::now();
                            }
                            Command::SignOut=>{
                                pending=None;let mut warning=None;if let Some(c)=credential.take(){let revoked=c.authorize(http.post(format!("{}/api/auth/logout",c.server))).header("Origin",&c.server).send().await.is_ok_and(|response|response.status().is_success());let removed=remove(&c.server).is_ok();if !revoked||!removed{warning=Some("Discord sign-out could not be fully confirmed. Check server access and OS credential storage.");}}
                                update(&shared,|s|{s.account=None;s.credential=None;s.busy=false;s.error.clear();});
                                if let Some(warning)=warning{set_error(&shared,warning);}
                            }
                            Command::Cancel=>{pending=None;update(&shared,|s|s.busy=false);}
                        }
                    }
                    if Instant::now()>=next{
                        if let Some((id,proof,deadline,generation))=&pending{
                            if Instant::now()>=*deadline{set_error(&shared,"Discord sign-in expired. Try again.");pending=None;update(&shared,|s|s.busy=false);}
                            else if let Ok(response)=http.post(format!("{server}/api/auth/discord/poll")).header("Origin",&server).json(&serde_json::json!({"requestId":id,"proof":proof})).send().await{
                                if response.status()==reqwest::StatusCode::OK{
                                    #[derive(Deserialize)]struct Issued{token:String,account:Account}
                                    if let Ok(bytes)=checked(response).await&&let Ok(issued)=serde_json::from_slice::<Issued>(&bytes)&&valid_token(&issued.token){
                                        let issued_credential = Credential { server: server.clone(), token: issued.token };
                                        let committed = shared.lock().is_ok_and(|mut state| {
                                            if state.attempt_generation != *generation || stopped.load(Ordering::Acquire) {
                                                return false;
                                            }
                                            if save(&issued_credential).is_err() {
                                                state.error = "Discord connected for this run, but the OS could not remember the login.".into();
                                            }
                                            state.account = Some(issued.account);
                                            state.credential = Some(issued_credential.clone());
                                            state.busy = false;
                                            true
                                        });
                                        pending = None;
                                        if committed {
                                            credential = Some(issued_credential);
                                        } else {
                                            let _ = issued_credential
                                                .authorize(http.post(format!("{server}/api/auth/logout")))
                                                .header("Origin", &server)
                                                .send()
                                                .await;
                                        }
                                    }
                                }else if response.status()==reqwest::StatusCode::UNAUTHORIZED{pending=None;set_error(&shared,"Discord sign-in was denied or expired. Try again.");update(&shared,|s|s.busy=false);}
                            }
                            next=Instant::now()+Duration::from_secs(2);
                        }else{
                            if let Ok(response)=http.get(format!("{server}/api/auth/config")).send().await&&let Ok(bytes)=checked(response).await{let available=serde_json::from_slice::<serde_json::Value>(&bytes).ok().and_then(|v|v["discord"].as_bool()).unwrap_or(false);update(&shared,|s|s.available=available);}
                            if let Some(c)=&credential&&let Ok(response)=c.authorize(http.get(format!("{server}/api/auth/session"))).send().await{
                                if response.status()==reqwest::StatusCode::UNAUTHORIZED{let _=remove(&server);credential=None;update(&shared,|s|{s.account=None;s.credential=None;});}
                                else if let Ok(bytes)=checked(response).await&&let Ok(account)=serde_json::from_slice::<Account>(&bytes){update(&shared,|s|{s.account=Some(account);s.credential=credential.clone();});}
                            }
                            next=Instant::now()+Duration::from_secs(20);
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            });
        }).expect("Discord login worker");
        Self {
            state,
            commands: Some(tx),
            browser,
            stop,
        }
    }
    #[cfg(test)]
    pub fn disabled() -> Self {
        Self {
            state: Arc::new(Mutex::new(State::default())),
            commands: None,
            browser: Arc::default(),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn state(&self) -> State {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }
    pub fn take_browser_url(&self) -> Option<String> {
        self.browser.lock().ok()?.take()
    }
    pub fn sign_in(&self, server: String) {
        if let (Some(tx), Ok(mut state)) = (&self.commands, self.state.lock()) {
            state.attempt_generation += 1;
            let _ = tx.send(Command::SignIn(server, state.attempt_generation));
        }
    }
    pub fn sign_out(&self) {
        if let Some(tx) = &self.commands {
            let _ = tx.send(Command::SignOut);
        }
    }
    pub fn cancel(&self) {
        update(&self.state, |state| {
            state.attempt_generation += 1;
            state.busy = false;
            if let Ok(mut url) = self.browser.lock() {
                *url = None;
            }
        });
        if let Some(tx) = &self.commands {
            let _ = tx.send(Command::Cancel);
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
fn update(state: &Mutex<State>, apply: impl FnOnce(&mut State)) {
    if let Ok(mut s) = state.lock() {
        apply(&mut s);
    }
}
fn set_error(state: &Mutex<State>, message: &str) {
    update(state, |s| s.error = message.into());
}
fn valid_token(token: &str) -> bool {
    token.len() == 43
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn entry(server: &str) -> Result<keyring::Entry> {
    ensure!(
        crate::admission::valid_server(server),
        "Invalid login server."
    );
    keyring::Entry::new("PodeTelar Discord v1", server)
        .map_err(|_| anyhow::anyhow!("OS credential storage unavailable."))
}
fn load(server: &str) -> Result<Option<Credential>> {
    match entry(server)?.get_password() {
        Ok(token) => {
            ensure!(valid_token(&token), "Invalid remembered login.");
            Ok(Some(Credential {
                server: server.into(),
                token,
            }))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => anyhow::bail!("Could not read remembered Discord login."),
    }
}
fn save(credential: &Credential) -> Result<()> {
    entry(&credential.server)?
        .set_password(&credential.token)
        .map_err(|_| anyhow::anyhow!("Could not remember Discord login."))
}
fn remove(server: &str) -> Result<()> {
    match entry(server)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => anyhow::bail!("Could not clear remembered Discord login."),
    }
}
async fn checked(mut response: reqwest::Response) -> Result<Vec<u8>> {
    ensure!(response.status().is_success(), "Account request denied.");
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .context("Account response unavailable.")?
    {
        ensure!(
            bytes.len() + chunk.len() <= 65536,
            "Account response too large."
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_are_only_selected_for_their_exact_backend() {
        let c = Credential {
            server: "https://one.example".into(),
            token: "a".repeat(43),
        };
        assert!(c.for_server("https://one.example").is_some());
        assert!(c.for_server("https://two.example").is_none());
        assert!(valid_token(&"a".repeat(43)));
        assert!(!valid_token("bad"));
    }
    #[cfg(windows)]
    #[test]
    fn windows_credential_store_round_trip_is_scoped_and_removable() {
        let server = format!("https://credential-test-{}.invalid", rand::random::<u64>());
        let credential = Credential {
            server: server.clone(),
            token: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(rand::random::<[u8; 32]>()),
        };
        let result = (|| -> Result<()> {
            save(&credential)?;
            let restored = load(&server)?.context("Test credential unavailable")?;
            ensure!(
                restored.for_server(&server) == credential.for_server(&server),
                "Credential changed"
            );
            Ok(())
        })();
        let removed = remove(&server);
        assert!(result.is_ok());
        assert!(removed.is_ok());
        assert!(load(&server).unwrap().is_none());
    }
}
