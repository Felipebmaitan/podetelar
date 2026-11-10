# Private stream previews

An unwatched tile receives a JPEG, not a video track. Publishers generate one still when capture starts and no more than one every 60 seconds after that. Joining, Watch/Stop watching, HTTP retries and a media reconnect do not request a new capture. A late viewer receives the latest available image immediately.

The native publisher letterboxes the source into 960×540, preserving aspect ratio,
and encodes baseline JPEG at quality 85. Uploads cannot exceed 256 KiB. The minute
gate runs before GPU scaling, download and JPEG encoding in a bounded branch of
the existing capture pipeline. Preview failure leaves a placeholder and never
starts a video subscription. An unchanged image revision may be fetched again
when returning from Watch; this does not request another capture.

The server retains only the latest image for each publisher/source in RAM. Nothing is written to a file, SQLite, a public object store, or a shared CDN. Images expire after three minutes, including after a failed uploader; a sweep runs every 30 seconds. The process caps storage at 256 recent publisher/source entries (64 MiB of images at maximum size), with at most 16 upload jobs. Stop releases image bytes immediately; its small cadence descriptor remains until expiry so DELETE cannot reset the same track's minute limit. It reads bounded JPEG headers/scan structure to enforce dimensions and a single baseline image, without decoding or transcoding pixels. Restarting the app clears all preview images; publishers regenerate on their normal minute clock.

## Access and notifications

- POST `/api/rooms/:roomId/previews/:kind` sends `image/jpeg` plus `X-Track-Sid`. The authenticated cookie fixes the publisher identity. Exact Origin validation and the API rate limit apply. A privileged SFU lookup must confirm that identity's current, unmuted screen/camera track matches the SID.
- A successful upload announces a bounded descriptor in the publisher's `freedomstream.preview.screen` or `.camera` participant attribute. This uses a server-side administrative RPC, not participant data-publish permission. Independent attribute keys avoid read/modify/write races between camera and screen. The descriptor is a notification, never image access authority.
- GET `/api/rooms/:roomId/previews/:publisherId/:kind/:revision` requires an admitted same-room session and rechecks the active, unmuted SFU publication. Stopped, muted, replaced or expired publications return no image. Responses are private and `no-store`; clients use short-lived blob URLs and revoke them on replacement/unmount.
- DELETE of the publisher's kind includes its SID. Stop removes matching bytes/cancels an in-flight upload; an old Stop cannot erase a replacement stream. Leave/rejoin also purges that session's previews. A crash or failed DELETE is covered by the GET live-publication check and expiry. The room UI removes tiles as publications disappear.

Images and uploader work share no camera/microphone acquisition, RTC subscription, or server video decoder. A watcher waiting for H.264 fallback can still identify the stream from its JPEG. A transient SFU outage denies preview access instead of serving unchecked cached content. Client retries reuse the same JPEG; new media SIDs reattach the cached image after full reconnect, preserving the capture clock.

The API's `LIVEKIT_INTERNAL_URL` is the administrative HTTP(S) origin reachable from the app container. It defaults to the private loopback `http://127.0.0.1:7880`; Compose explicitly uses `http://livekit:7880`. Only the server holds the administrative key/secret. Public signaling now derives from `PUBLIC_URL` and passes through the app admission gateway; the SFU HTTP listener stays private.
