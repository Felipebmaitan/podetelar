use crate::protocol::{self, Action, Phase, Request, Status};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    net::{Ipv4Addr, TcpListener},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use velopack::{UpdateCheck, UpdateInfo, UpdateManager, UpdateOptions, sources};

enum Work {
    Action(Action),
    Exited(mpsc::Sender<bool>),
}

pub struct Bridge {
    pub address: String,
    pub token: String,
    work: mpsc::SyncSender<Work>,
    status: Arc<Mutex<Status>>,
}

fn manager() -> Option<(UpdateManager, std::path::PathBuf)> {
    let feed = option_env!("FREEDOMSTREAM_UPDATE_FEED")?;
    let source: Box<dyn sources::UpdateSource> = if std::path::Path::new(feed).is_absolute() {
        Box::new(sources::FileSource::new(feed))
    } else {
        let url = url::Url::parse(feed).ok()?;
        // Feed selection is baked into this build, never supplied by IPC/room data.
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return None;
        }
        if url.host_str() == Some("github.com") {
            Box::new(sources::GithubSource::new(feed, None, false))
        } else {
            Box::new(sources::HttpSource::new_with_options(
                feed,
                velopack::HttpOptions {
                    TimeoutMilliseconds: 180_000,
                    ..Default::default()
                },
            ))
        }
    };
    let packages = velopack::locator::auto_locate_app_manifest(
        velopack::locator::LocationContext::FromCurrentExe,
    )
    .ok()?
    .get_packages_dir();
    let manager = UpdateManager::new_boxed(
        source,
        Some(UpdateOptions {
            AllowVersionDowngrade: false,
            ExplicitChannel: None,
            MaximumDeltasBeforeFallback: -1,
        }),
        None,
    )
    .ok()?;
    Some((manager, packages))
}

fn full_only(mut found: UpdateInfo) -> UpdateInfo {
    // The pinned Rust SDK does not enforce MaximumDeltasBeforeFallback.
    found.BaseRelease = None;
    found.DeltasToTarget.clear();
    found
}

fn verified(path: &std::path::Path, asset: &velopack::VelopackAsset) -> bool {
    let check = || -> std::io::Result<bool> {
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1); // No writes/replacements while hashing.
        }
        let mut file = options.open(path)?;
        if file.metadata()?.len() != asset.Size {
            return Ok(false);
        }
        let mut digest = Sha256::new();
        let mut buffer = [0; 65536];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
        }
        let actual = digest
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        Ok(actual.eq_ignore_ascii_case(&asset.SHA256))
    };
    check().unwrap_or(false)
}

impl Bridge {
    pub fn start() -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?.to_string();
        let token = rand::random::<[u8; 32]>()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let status = Arc::new(Mutex::new(Status::default()));
        let (work, receive) = mpsc::sync_channel(4);
        let server_work = work.clone();
        let server_status = status.clone();
        let server_token = token.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else {
                    break;
                };
                let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
                let _ = stream.set_write_timeout(Some(Duration::from_millis(300)));
                let Ok(request) = protocol::read::<Request>(&mut stream) else {
                    continue;
                };
                if request.token != server_token {
                    continue;
                }
                if request.action != Action::Poll {
                    let _ = server_work.try_send(Work::Action(request.action));
                }
                let _ = protocol::write(&mut stream, &*server_status.lock().unwrap());
            }
        });
        let worker_status = status.clone();
        std::thread::spawn(move || worker(worker_status, receive));
        Ok(Self {
            address,
            token,
            work,
            status,
        })
    }

    /// Called only after the portable process has exited and closed its media job.
    pub fn child_exited(&self) -> bool {
        if self.status.lock().unwrap().phase != Phase::Restarting {
            return true;
        }
        let (reply, result) = mpsc::channel();
        self.work.try_send(Work::Exited(reply)).is_ok()
            && result.recv_timeout(Duration::from_secs(5)).unwrap_or(false)
    }
}

fn worker(status: Arc<Mutex<Status>>, receive: mpsc::Receiver<Work>) {
    let Some((manager, packages)) = manager() else {
        status.lock().unwrap().message = "Updates are not configured for this build.".into();
        return;
    };
    let mut update: Option<UpdateInfo> = None;
    // Startup check and bounded periodic checks. User downloads/restarts explicitly.
    let mut next = Action::Check;
    loop {
        match next {
            Action::Check => {
                status.lock().unwrap().phase = Phase::Checking;
                match manager.check_for_updates() {
                    Ok(UpdateCheck::UpdateAvailable(found)) if !found.IsDowngrade => {
                        let asset = &found.TargetFullRelease;
                        if asset.Size == 0
                            || asset.Size > 1024 * 1024 * 1024
                            || asset.Version.len() > 80
                            || asset.PackageId != manager.get_app_id()
                            || !asset.Type.eq_ignore_ascii_case("full")
                            || !asset.FileName.ends_with(".nupkg")
                            || asset.FileName.contains(['/', '\\', ':'])
                            || asset.SHA256.len() != 64
                            || !asset.SHA256.bytes().all(|c| c.is_ascii_hexdigit())
                            || (!manager.get_current_version_as_string().contains('-')
                                && asset.Version.contains('-'))
                        {
                            status.lock().unwrap().phase = Phase::Error;
                            status.lock().unwrap().message =
                                "The update package is not supported.".into();
                        } else {
                            *status.lock().unwrap() = Status {
                                phase: Phase::Available,
                                version: asset.Version.clone(),
                                notes: protocol::notes(&asset.NotesMarkdown),
                                ..Default::default()
                            };
                            update = Some(full_only(*found));
                        }
                    }
                    Ok(_) => {
                        update = None;
                        *status.lock().unwrap() = Status {
                            phase: Phase::Current,
                            ..Default::default()
                        };
                    }
                    Err(_) => {
                        status.lock().unwrap().phase = Phase::Error;
                        status.lock().unwrap().message =
                            "Could not check for updates. Try again.".into();
                    }
                }
            }
            Action::Download if status.lock().unwrap().phase == Phase::Available => {
                if let Some(found) = &update {
                    status.lock().unwrap().phase = Phase::Downloading;
                    let (progress, reports) = mpsc::channel::<i16>();
                    let copy = status.clone();
                    let reporter = std::thread::spawn(move || {
                        for percent in reports {
                            copy.lock().unwrap().progress = percent.clamp(0, 100) as u8;
                        }
                    });
                    let path = packages.join(&found.TargetFullRelease.FileName);
                    let result = (|| -> Result<(), velopack::Error> {
                        // SDK download_updates returns Ok without checking existing files.
                        if path.exists() && !verified(&path, &found.TargetFullRelease) {
                            std::fs::remove_file(&path)?;
                        }
                        manager.download_updates(found, Some(progress))?;
                        if !verified(&path, &found.TargetFullRelease) {
                            return Err(
                                std::io::Error::from(std::io::ErrorKind::InvalidData).into()
                            );
                        }
                        Ok(())
                    })();
                    let _ = reporter.join();
                    let mut state = status.lock().unwrap();
                    state.phase = if result.is_ok() {
                        Phase::Ready
                    } else {
                        Phase::Error
                    };
                    state.message = if result.is_ok() {
                        String::new()
                    } else {
                        "Could not download or verify the update. Try again.".into()
                    };
                }
            }
            Action::Restart if status.lock().unwrap().phase == Phase::Ready => {
                status.lock().unwrap().phase = Phase::Restarting;
            }
            _ => {}
        }
        loop {
            // Checks do not replace a downloaded package or an explicit restart intent.
            match receive.recv_timeout(Duration::from_secs(30 * 60 + rand::random_range(0..120))) {
                Ok(Work::Exited(reply)) => {
                    let requested = status.lock().unwrap().phase == Phase::Restarting;
                    let applied = requested
                        && update.as_ref().is_some_and(|found| {
                            verified(
                                &packages.join(&found.TargetFullRelease.FileName),
                                &found.TargetFullRelease,
                            ) && manager
                                .wait_exit_then_apply_updates(
                                    found,
                                    true,
                                    true,
                                    std::iter::empty::<&str>(),
                                )
                                .is_ok()
                        });
                    let _ = reply.send(applied);
                    return;
                }
                Ok(Work::Action(action)) => {
                    let phase = status.lock().unwrap().phase;
                    if matches!(phase, Phase::Ready | Phase::Restarting)
                        && action != Action::Restart
                    {
                        continue;
                    }
                    next = action;
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if matches!(status.lock().unwrap().phase, Phase::Current | Phase::Error) {
                        next = Action::Check;
                        break;
                    }
                }
                Err(_) => return,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepted_update_discards_all_delta_assets() {
        let full = full_only(UpdateInfo {
            BaseRelease: Some(Default::default()),
            DeltasToTarget: vec![velopack::VelopackAsset {
                FileName: "../untrusted-delta.nupkg".into(),
                ..Default::default()
            }],
            ..Default::default()
        });
        assert!(full.BaseRelease.is_none());
        assert!(full.DeltasToTarget.is_empty());
    }
}
