// PodeTelar regression test, MIT licensed. Appended to the patched SDK.
#[cfg(test)]
mod podetelar_encryption_receiver_tests {
    use super::*;
    use libwebrtc::video_source::{RtcVideoSource, VideoResolution, native::NativeVideoSource};
    use std::{sync::mpsc, thread, time::Duration};

    #[tokio::test]
    async fn protected_peer_connection_requires_its_own_guarded_call() {
        use libwebrtc::peer_connection_factory::{PeerConnectionFactory, RtcConfiguration};
        let factory = PeerConnectionFactory::default();
        let legacy = factory.create_peer_connection(RtcConfiguration::default()).unwrap();
        let mut private = RtcConfiguration::default();
        private.protect_receive_streams = true;
        // The native factory returns an error unless this specific connection's
        // underlying Call acknowledged the guarded receive-stream constructor.
        let first = factory.create_peer_connection(private.clone()).unwrap();
        let second = factory.create_peer_connection(private).unwrap();
        drop((first, second, legacy));
    }

    fn receiver(name: &str) -> RemoteTrack {
        let source = RtcVideoSource::Native(NativeVideoSource::new(
            VideoResolution {
                width: 16,
                height: 16,
            },
            false,
        ));
        let local = LocalVideoTrack::create_video_track(name, source);
        RemoteTrack::Video(RemoteVideoTrack::new(
            "TR_serialized".to_owned().try_into().unwrap(),
            name.into(),
            local.rtc_track(),
        ))
    }

    #[tokio::test]
    async fn same_sid_replacements_wait_for_subscription_readiness() {
        let publication = RemoteTrackPublication::new(
            proto::TrackInfo {
                sid: "TR_serialized".into(),
                r#type: proto::TrackType::Video as i32,
                ..Default::default()
            },
            None,
            false,
        );
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Mutex::new(release_rx);
        publication.on_subscribed(move |publication, track| {
            // Encryption is prepared directly by the attachment path. This
            // notification must finish before either replacement is visible.
            assert!(publication.track().is_none());
            entered_tx.send(track.name()).unwrap();
            if track.name() == "first" {
                release_rx
                    .lock()
                    .recv_timeout(Duration::from_secs(3))
                    .unwrap();
            }
        });
        let first = receiver("first");
        let second = receiver("second");
        let a = publication.clone();
        let first_thread = thread::spawn(move || a.set_track(Some(first)));
        assert_eq!(
            entered_rx.recv_timeout(Duration::from_secs(3)).unwrap(),
            "first"
        );
        assert!(publication.track().is_none());
        let b = publication.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let second_thread = thread::spawn(move || {
            started_tx.send(()).unwrap();
            b.set_track(Some(second));
        });
        started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        thread::sleep(Duration::from_millis(25));
        assert!(publication.track().is_none());
        assert!(entered_rx.try_recv().is_err());
        release_tx.send(()).unwrap();
        first_thread.join().unwrap();
        second_thread.join().unwrap();
        assert_eq!(
            entered_rx.recv_timeout(Duration::from_secs(3)).unwrap(),
            "second"
        );
        assert_eq!(publication.track().unwrap().name(), "second");
    }
}
