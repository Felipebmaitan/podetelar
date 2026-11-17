//! Profile HTTP and image decoding stay off the GUI and media workers.
use crate::admission::Admission;
use anyhow::{Context, Result, ensure};
use eframe::egui;
use image::{AnimationDecoder, ImageDecoder};
use serde::Deserialize;
use std::{
    collections::HashMap,
    io::{Cursor, Read},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    #[serde(default)]
    pub discord: bool,
    pub participant_id: String,
    pub display_name: String,
    pub avatar_revision: Option<String>,
    pub uploads_blocked: bool,
}
pub struct Animation {
    pub frames: Vec<(egui::ColorImage, Duration)>,
    pub duration: Duration,
}
impl Animation {
    pub fn frame(&self, elapsed: Duration) -> usize {
        let mut elapsed = elapsed.as_millis() % self.duration.as_millis().max(1);
        for (index, (_, delay)) in self.frames.iter().enumerate() {
            if elapsed < delay.as_millis() {
                return index;
            }
            elapsed = elapsed.saturating_sub(delay.as_millis());
        }
        0
    }
}

pub fn decode(bytes: &[u8]) -> Result<Animation> {
    ensure!(bytes.len() <= 3_000_000, "Profile picture too large.");
    let mut frames = Vec::new();
    let mut duration = Duration::ZERO;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(128);
    limits.max_image_height = Some(128);
    limits.max_alloc = Some(64 * 1024 * 1024);
    let mut add = |pixels: image::RgbaImage, delay: Duration| -> Result<()> {
        ensure!(frames.len() < 200, "Too many profile picture frames.");
        let image = image::imageops::resize(&pixels, 64, 64, image::imageops::FilterType::Triangle);
        frames.push((
            egui::ColorImage::from_rgba_unmultiplied([64, 64], image.as_raw()),
            delay,
        ));
        duration += delay;
        Ok(())
    };
    if bytes.starts_with(b"GIF8") {
        let mut decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes))?;
        decoder.set_limits(limits)?;
        for frame in decoder.into_frames() {
            let frame = frame?;
            let (n, d) = frame.delay().numer_denom_ms();
            let delay =
                Duration::from_millis((u64::from(n) / u64::from(d.max(1))).clamp(20, 10000));
            add(frame.into_buffer(), delay)?;
        }
    } else {
        let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
        reader.limits(limits);
        add(reader.decode()?.into_rgba8(), Duration::from_secs(1))?;
    }
    ensure!(!frames.is_empty(), "Empty profile picture.");
    Ok(Animation { frames, duration })
}

pub enum Edit {
    Discord(Option<crate::account::Credential>),
    Name(String),
    Picture(Vec<u8>),
    Remove,
}
enum Command {
    Edit(Edit),
    Choose,
}
#[derive(Clone, Default)]
pub struct State {
    pub profiles: Vec<Participant>,
    pub avatars: HashMap<String, Arc<Animation>>,
    pub error: String,
    pub busy: bool,
}
pub struct Client {
    pub participant_id: String,
    pub server: String,
    state: Arc<Mutex<State>>,
    commands: mpsc::Sender<Command>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Client {
    pub fn new(admission: Admission) -> Self {
        let participant_id = admission.participant_id.clone();
        let server = admission.public_origin.clone();
        let state = Arc::new(Mutex::new(State::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (commands, receiver) = mpsc::channel();
        let worker_state = state.clone();
        let stopped = stop.clone();
        let worker = std::thread::Builder::new()
            .name("podetelar-profiles".into())
            .spawn(move || {
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                runtime.block_on(async move {
                    let mut next = Instant::now();
                    let mut avatars: HashMap<String, Arc<Animation>> = HashMap::new();
                    while !stopped.load(Ordering::Acquire) {
                        if let Ok(command) = receiver.try_recv() {
                            if let Ok(mut state) = worker_state.lock() {
                                state.busy = true;
                                state.error.clear();
                            }
                            let edit = match command {
                                Command::Edit(edit) => Ok(Some(edit)),
                                Command::Choose => {
                                    choose_picture().map(|bytes| bytes.map(Edit::Picture))
                                }
                            };
                            let result = match edit {
                                Ok(Some(edit)) => admission.edit_profile(edit).await,
                                Ok(None) => Ok(()),
                                Err(error) => Err(error),
                            };
                            if let Ok(mut state) = worker_state.lock() {
                                state.busy = false;
                                if let Err(error) = result {
                                    state.error = error.to_string();
                                }
                            }
                            next = Instant::now();
                        }
                        if Instant::now() >= next {
                            if let Ok(profiles) = admission.profiles().await {
                                avatars.retain(|revision, _| {
                                    profiles
                                        .iter()
                                        .any(|p| p.avatar_revision.as_ref() == Some(revision))
                                });
                                // Publish removals before fetching new pictures, including administrator deletion.
                                if let Ok(mut state) = worker_state.lock() {
                                    state.profiles = profiles.clone();
                                    state.avatars = avatars.clone();
                                }
                                for participant in &profiles {
                                    if stopped.load(Ordering::Acquire) {
                                        break;
                                    }
                                    if let Some(revision) = &participant.avatar_revision
                                        && !avatars.contains_key(revision)
                                        && let Ok(bytes) = admission
                                            .profile_avatar(&participant.participant_id, revision)
                                            .await
                                        && let Ok(image) = decode(&bytes)
                                    {
                                        avatars.insert(revision.clone(), Arc::new(image));
                                    }
                                }
                                if let Ok(mut state) = worker_state.lock() {
                                    state.avatars = avatars.clone();
                                }
                            } else if let Ok(mut state) = worker_state.lock() {
                                state.avatars.clear();
                            }
                            next = Instant::now() + Duration::from_secs(3);
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                });
            })
            .expect("profile worker");
        Self {
            participant_id,
            server,
            state,
            commands,
            stop,
            worker: Some(worker),
        }
    }
    pub fn state(&self) -> State {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }
    pub fn name(&self, name: String) {
        let _ = self.commands.send(Command::Edit(Edit::Name(name)));
    }
    pub fn connect(&self, credential: Option<crate::account::Credential>) {
        let _ = self.commands.send(Command::Edit(Edit::Discord(credential)));
    }
    pub fn choose(&self) {
        let _ = self.commands.send(Command::Choose);
    }
    pub fn remove(&self) {
        let _ = self.commands.send(Command::Edit(Edit::Remove));
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if self.worker.as_ref().is_some_and(crate::worker::finished) {
            let _ = self.worker.take().unwrap().join();
        }
    }
}
fn choose_picture() -> Result<Option<Vec<u8>>> {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Profile pictures", &["png", "jpg", "jpeg", "webp", "gif"])
        .pick_file()
    else {
        return Ok(None);
    };
    let file = std::fs::File::open(path).context("Could not open profile picture.")?;
    ensure!(
        file.metadata()?.len() <= 3_000_000,
        "Profile pictures must be 3 MB or smaller."
    );
    let mut bytes = Vec::new();
    file.take(3_000_001).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 3_000_000,
        "Profile pictures must be 3 MB or smaller."
    );
    Ok(Some(bytes))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Qualification {
    invitation_url: String,
    display_name: String,
    new_name: String,
    upload: bool,
}

/// Explicit stdin-only acceptance, using real native admission, SFU and image decoding.
pub async fn qualify(request: Qualification) -> Result<()> {
    let admission = Admission::join(&request.invitation_url, &request.display_name).await?;
    let access = admission.media_access().await?;
    let (room, _events) = livekit::Room::connect(
        &access.server_url,
        &access.token,
        crate::connection::options(crate::connection::Route::Auto),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Profile fixture media connection failed."))?;
    let result:Result<_>=async{
        admission.edit_profile(Edit::Name(request.new_name.clone())).await?;
        if request.upload {
            let mut bytes=Vec::new();{
                let mut encoder=image::codecs::gif::GifEncoder::new(&mut bytes);
                encoder.set_repeat(image::codecs::gif::Repeat::Infinite)?;
                for color in [[255,0,0,255],[0,255,0,255]]{encoder.encode_frame(image::Frame::from_parts(image::RgbaImage::from_pixel(2,2,image::Rgba(color)),0,0,image::Delay::from_numer_denom_ms(100,1)))?;}
            }
            admission.edit_profile(Edit::Picture(bytes)).await?;
        }
        for _ in 0..50{
            if room.local_participant().name()==request.new_name{break;}
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        ensure!(room.local_participant().name()==request.new_name,"SFU nickname did not synchronize.");
        let profiles=admission.profiles().await?;
        let own=profiles.iter().find(|p|p.participant_id==admission.participant_id).context("Profile fixture participant missing.")?;
        let revision=own.avatar_revision.as_ref().context("Profile fixture picture missing.")?;
        let bytes=admission.profile_avatar(&admission.participant_id,revision).await?;
        let image=decode(&bytes)?;
        ensure!(image.frames.len()==2,"Profile animation frames missing.");
        Ok(serde_json::json!({"displayName":own.display_name,"mediaName":room.local_participant().name(),"decodedFrames":image.frames.len(),"animated":image.frames[0].0.pixels!=image.frames[1].0.pixels}))
    }.await;
    let _ = room.close().await;
    admission.leave().await;
    println!("{}", result?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn animated_gif_decodes_frames_with_timing() {
        let mut bytes = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
            for color in [[255, 0, 0, 255], [0, 255, 0, 255]] {
                encoder
                    .encode_frame(image::Frame::from_parts(
                        image::RgbaImage::from_pixel(2, 2, image::Rgba(color)),
                        0,
                        0,
                        image::Delay::from_numer_denom_ms(100, 1),
                    ))
                    .unwrap();
            }
        }
        let animation = decode(&bytes).unwrap();
        assert_eq!(animation.frames.len(), 2);
        assert_eq!(animation.frame(Duration::from_millis(150)), 1);
        assert_eq!(animation.frame(Duration::from_millis(250)), 0);
        assert_ne!(animation.frames[0].0.pixels, animation.frames[1].0.pixels);
    }
    #[test]
    fn malformed_and_oversized_images_are_rejected() {
        assert!(decode(b"not an image").is_err());
        assert!(decode(&vec![0; 3_000_001]).is_err());
    }
}
