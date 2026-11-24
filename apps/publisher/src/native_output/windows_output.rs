use anyhow::{Context, Result};
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::AppSrc;
use std::time::{Duration, Instant};

pub(super) struct Output {
    pipeline: crate::playback::AudioPipeline,
    src: AppSrc,
    sink: gst::Element,
    bus: gst::Bus,
    route: String,
    rendered: u64,
    progress: Instant,
}

impl Output {
    pub(super) fn start(route: &str) -> Result<Self> {
        let pipeline = gst::parse::launch(
            "appsrc name=pcm is-live=true format=time do-timestamp=true block=false \
             max-buffers=4 max-bytes=0 leaky-type=downstream emit-signals=false ! \
             audio/x-raw,format=S16LE,rate=48000,channels=2,layout=interleaved ! \
             audioconvert ! audioresample ! wasapi2sink name=output exclusive=false \
             provide-clock=false low-latency=true buffer-time=40000 sync=false",
        )?
        .downcast::<gst::Pipeline>()
        .ok()
        .context("Invalid Windows playback pipeline.")?;
        let pipeline = crate::playback::AudioPipeline(pipeline);
        let src = pipeline
            .0
            .by_name("pcm")
            .and_downcast::<AppSrc>()
            .context("Missing Windows playback input.")?;
        let sink = pipeline
            .0
            .by_name("output")
            .context("Missing Windows playback sink.")?;
        sink.set_property("device", route);
        let bus = pipeline.0.bus().context("Missing Windows playback bus.")?;
        pipeline.0.set_state(gst::State::Playing)?;
        Ok(Self {
            pipeline,
            src,
            sink,
            bus,
            route: route.to_owned(),
            rendered: 0,
            progress: Instant::now(),
        })
    }

    pub(super) fn appsrc(&self) -> *mut u8 {
        self.src.as_ptr().cast()
    }

    pub(super) fn route(&self) -> &str {
        &self.route
    }

    // None means the sink failed or stopped consuming PCM. An asynchronous
    // transition to Playing alone is insufficient to report working output.
    pub(super) fn poll(&mut self, now: Instant) -> Option<bool> {
        if self
            .bus
            .pop_filtered(&[gst::MessageType::Error, gst::MessageType::Eos])
            .is_some()
        {
            return None;
        }
        let rendered = self
            .sink
            .property::<gst::Structure>("stats")
            .get::<u64>("rendered")
            .unwrap_or(0);
        if rendered != self.rendered {
            self.rendered = rendered;
            self.progress = now;
        }
        (now.saturating_duration_since(self.progress) < Duration::from_secs(3))
            .then(|| self.rendered > 0 && self.pipeline.0.current_state() == gst::State::Playing)
    }
}
