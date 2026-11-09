use crate::{admission, win32::Handle};
use anyhow::{Context, Result, bail, ensure};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tempfile::TempDir;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};
use windows::{
    Win32::System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
            QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
        },
        Threading::{
            CREATE_NO_WINDOW, CREATE_SUSPENDED, CreateProcessW, PROCESS_INFORMATION, ResumeThread,
            STARTUPINFOW, TerminateProcess,
        },
    },
    core::{PCWSTR, PWSTR},
};

pub struct Viewer {
    _process: Option<Handle>,
    job: Handle,
    // Field order keeps the job (and browser processes) alive until before the
    // ephemeral profile is deleted. The profile never holds normal browser data.
    profile: Option<TempDir>,
    connection: Option<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>>,
}

impl Drop for Viewer {
    fn drop(&mut self) {
        self.connection.take();
        unsafe {
            let _ = TerminateJobObject(self.job.0, 0);
        }
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(2) {
            let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
            if unsafe {
                QueryInformationJobObject(
                    Some(self.job.0),
                    JobObjectBasicAccountingInformation,
                    &mut accounting as *mut _ as *mut _,
                    std::mem::size_of_val(&accounting) as u32,
                    None,
                )
            }
            .is_err()
                || accounting.ActiveProcesses == 0
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self._process.take();
        if let Some(profile) = self.profile.take() {
            let directory = profile.path().to_owned();
            if profile.close().is_err() {
                // Windows scanner/mapping teardown can briefly outlive the job.
                // Retry only this generated, resolved directory in the temp root.
                let safe = directory.canonicalize().ok().filter(|path| {
                    path.file_name().is_some_and(|name| {
                        name.to_string_lossy().starts_with("FreedomStream-viewer-")
                    }) && std::env::temp_dir()
                        .canonicalize()
                        .is_ok_and(|root| path.parent() == Some(root.as_path()))
                });
                if let Some(directory) = safe {
                    let started = Instant::now();
                    while std::fs::remove_dir_all(&directory).is_err()
                        && directory.exists()
                        && started.elapsed() < Duration::from_secs(2)
                    {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        }
    }
}

impl Viewer {
    pub async fn open(invitation: &str, headless: bool) -> Result<Self> {
        let url = admission::invitation_url(invitation)?;
        let executable =
            chrome().context("Install Google Chrome to use the isolated room viewer.")?;
        let profile = tempfile::Builder::new()
            .prefix("FreedomStream-viewer-")
            .tempdir()
            .context("Could not create the private viewer profile.")?;
        let arguments = format!(
            "{} --user-data-dir={} --remote-debugging-port=0 --remote-debugging-address=127.0.0.1 --no-first-run --no-default-browser-check --disable-extensions --disable-sync --incognito {} --app=about:blank",
            quote(&executable),
            quote(profile.path()),
            if headless {
                "--headless=new --autoplay-policy=no-user-gesture-required"
            } else {
                ""
            }
        );
        let mut command: Vec<u16> = arguments.encode_utf16().chain(Some(0)).collect();
        let application: Vec<u16> = executable
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let mut info = PROCESS_INFORMATION::default();
        let startup = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let job = Handle(unsafe { CreateJobObjectW(None, PCWSTR::null()) }?);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            )
        }?;
        unsafe {
            CreateProcessW(
                PCWSTR(application.as_ptr()),
                Some(PWSTR(command.as_mut_ptr())),
                None,
                None,
                false,
                CREATE_SUSPENDED | CREATE_NO_WINDOW,
                None,
                PCWSTR::null(),
                &startup,
                &mut info,
            )
        }
        .context("Could not open the isolated room viewer.")?;
        let process = Handle(info.hProcess);
        let thread = Handle(info.hThread);
        if unsafe { AssignProcessToJobObject(job.0, process.0) }.is_err() {
            unsafe {
                let _ = TerminateProcess(process.0, 1);
            }
            bail!("Could not isolate the room viewer process.");
        }
        // Start suspended, assign the job, then resume: even the earliest Chrome
        // children inherit ownership and are closed with the publisher.
        ensure!(
            unsafe { ResumeThread(thread.0) } != u32::MAX,
            "Could not start the room viewer."
        );
        let mut viewer = Self {
            _process: Some(process),
            job,
            profile: Some(profile),
            connection: None,
        };
        viewer.navigate(url.as_str()).await?;
        Ok(viewer)
    }

    async fn navigate(&mut self, url: &str) -> Result<()> {
        let started = Instant::now();
        let port_file = self
            .profile
            .as_ref()
            .context("The viewer was closed.")?
            .path()
            .join("DevToolsActivePort");
        let port = loop {
            if let Ok(content) = std::fs::read_to_string(&port_file)
                && content.len() <= 1024
                && let Some(port) = content
                    .lines()
                    .next()
                    .and_then(|value| value.parse::<u16>().ok())
            {
                break port;
            }
            ensure!(
                started.elapsed() < Duration::from_secs(10),
                "The room viewer did not start. Check your Chrome installation."
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()?;
        let pages: Vec<Value> = client
            .get(format!("http://127.0.0.1:{port}/json/list"))
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Could not reach the isolated viewer."))?
            .json()
            .await
            .map_err(|_| anyhow::anyhow!("Invalid viewer response."))?;
        let socket = pages
            .iter()
            .find(|page| page["type"] == "page")
            .and_then(|page| page["webSocketDebuggerUrl"].as_str())
            .context("The room viewer did not create a page.")?;
        let address = url::Url::parse(socket).context("Invalid viewer address.")?;
        ensure!(
            address.scheme() == "ws"
                && address.host_str() == Some("127.0.0.1")
                && address.port() == Some(port),
            "Invalid local viewer address."
        );
        let (mut connection, _) = tokio_tungstenite::connect_async(socket)
            .await
            .map_err(|_| anyhow::anyhow!("Could not connect to the isolated viewer."))?;
        send(&mut connection, 1, "Page.navigate", json!({"url":url})).await?;
        self.connection = Some(connection);
        Ok(())
    }

    pub async fn qualification_tone(&mut self) -> Result<()> {
        let connection = self
            .connection
            .as_mut()
            .context("The viewer disconnected.")?;
        let started = Instant::now();
        let mut id = 2;
        loop {
            let ready = send(connection, id, "Runtime.evaluate", json!({"expression":"location.pathname.startsWith('/r/') && document.readyState === 'complete'","returnByValue":true})).await?;
            id += 1;
            if ready["result"]["result"]["value"] == true {
                break;
            }
            ensure!(
                started.elapsed() < Duration::from_secs(5),
                "The viewer page did not load."
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let result = send(connection, id, "Runtime.evaluate", json!({"expression":"window.fixtureAudio = new AudioContext(); const tone = fixtureAudio.createOscillator(); const gain = fixtureAudio.createGain(); tone.frequency.value = 1320; gain.gain.value = 0.02; tone.connect(gain); gain.connect(fixtureAudio.destination); tone.start(); fixtureAudio.resume(); 'started'","returnByValue":true})).await?;
        ensure!(
            result["result"]["result"]["value"] == "started",
            "Viewer tone did not start."
        );
        Ok(())
    }
}

async fn send(
    connection: &mut WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    id: u64,
    method: &str,
    params: Value,
) -> Result<Value> {
    connection
        .send(Message::Text(
            json!({"id":id,"method":method,"params":params})
                .to_string()
                .into(),
        ))
        .await
        .map_err(|_| anyhow::anyhow!("The room viewer disconnected."))?;
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(message) = connection.next().await {
            let message = message.map_err(|_| anyhow::anyhow!("The room viewer disconnected."))?;
            if let Message::Text(text) = message {
                ensure!(text.len() <= 65536, "Viewer response was too large.");
                let response: Value =
                    serde_json::from_str(&text).context("Invalid viewer response.")?;
                if response["id"] == id {
                    ensure!(
                        response["error"].is_null(),
                        "The room viewer could not navigate."
                    );
                    return Ok(response);
                }
            }
        }
        bail!("The room viewer disconnected.")
    })
    .await
    .context("The room viewer did not respond.")?
}

use std::os::windows::ffi::OsStrExt;
fn quote(path: &Path) -> String {
    format!("\"{}\"", path.display())
}
fn chrome() -> Option<PathBuf> {
    ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(|base| PathBuf::from(base).join("Google/Chrome/Application/chrome.exe"))
        .find(|path| path.is_file())
}
