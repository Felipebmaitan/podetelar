import {
  PREVIEW_MAX_BYTES,
  PREVIEW_MAX_HEIGHT,
  PREVIEW_MAX_WIDTH,
  THUMBNAIL_INTERVAL_MS,
  type PreviewKind,
} from "@freedomstream/contracts";
import { ApiError, request } from "../api";

type Job = {
  abort: AbortController;
  timer: ReturnType<typeof setTimeout> | null;
  lastCapture: number;
  image: Blob | null;
  uploaded: Blob | null;
  sid: string;
  retryAt: number;
};

/** A still-image side channel over the existing app-owned capture. Never subscribes. */
export class BrowserPreviews {
  private readonly jobs = new Map<MediaStreamTrack, Job>();
  constructor(private readonly roomId: string) {}

  start(
    track: MediaStreamTrack,
    kind: PreviewKind,
    currentSid: () => string | undefined,
  ): void {
    if (
      typeof document === "undefined" ||
      track.readyState !== "live" ||
      this.jobs.has(track)
    )
      return;
    const job: Job = {
      abort: new AbortController(),
      timer: null,
      lastCapture: -Infinity,
      image: null,
      uploaded: null,
      sid: "",
      retryAt: 0,
    };
    this.jobs.set(track, job);
    const tick = async () => {
      if (job.abort.signal.aborted || track.readyState === "ended") return;
      try {
        if (performance.now() - job.lastCapture >= THUMBNAIL_INTERVAL_MS) {
          // Retry delivery of this image, never recapture in response to an HTTP
          // error, a new viewer, or a changed publication SID during reconnect.
          job.lastCapture = performance.now();
          job.image = await captureStill(track, job.abort.signal);
          job.lastCapture = performance.now();
        }
        const sid = currentSid();
        if (
          sid &&
          job.image &&
          performance.now() >= job.retryAt &&
          (job.uploaded !== job.image || job.sid !== sid)
        ) {
          await request(`/api/rooms/${this.roomId}/previews/${kind}`, {
            method: "POST",
            headers: { "Content-Type": "image/jpeg", "X-Track-Sid": sid },
            body: job.image,
            signal: AbortSignal.any([
              job.abort.signal,
              AbortSignal.timeout(5000),
            ]),
          });
          job.sid = sid;
          job.uploaded = job.image;
        }
      } catch (error) {
        if (error instanceof ApiError && [401, 403].includes(error.status))
          this.stop(track);
        else
          job.retryAt =
            performance.now() +
            (error instanceof ApiError && error.retryAfterMs
              ? error.retryAfterMs
              : 3000);
        // Preview failure leaves a placeholder; live media keeps flowing.
      } finally {
        if (!job.abort.signal.aborted)
          job.timer = setTimeout(() => void tick(), 1000);
      }
    };
    void tick();
    // Keep deletion paired with this job, including the last SID after republish.
    job.abort.signal.addEventListener(
      "abort",
      () => {
        if (job.timer !== null) clearTimeout(job.timer);
        for (const sid of new Set(
          [job.sid, currentSid()].filter((value): value is string =>
            Boolean(value),
          ),
        ))
          void request(`/api/rooms/${this.roomId}/previews/${kind}`, {
            method: "DELETE",
            headers: { "X-Track-Sid": sid },
            signal: AbortSignal.timeout(3000),
          }).catch(() => {});
        job.image = job.uploaded = null;
      },
      { once: true },
    );
  }
  stop(track: MediaStreamTrack): void {
    this.jobs.get(track)?.abort.abort();
    this.jobs.delete(track);
  }
}

async function captureStill(
  track: MediaStreamTrack,
  parent: AbortSignal,
): Promise<Blob> {
  const signal = AbortSignal.any([parent, AbortSignal.timeout(5000)]);
  const video = document.createElement("video");
  video.muted = true;
  video.playsInline = true;
  video.srcObject = new MediaStream([track]);
  try {
    await new Promise<void>((resolve, reject) => {
      const done = (error?: unknown) => {
        video.removeEventListener("loadeddata", loaded);
        signal.removeEventListener("abort", aborted);
        error ? reject(error) : resolve();
      };
      const loaded = () => done();
      const aborted = () => done(new Error("Preview cancelled"));
      signal.addEventListener("abort", aborted, { once: true });
      video.addEventListener("loadeddata", loaded, { once: true });
      if (signal.aborted) aborted();
      else void video.play().catch((error: unknown) => done(error));
    });
    signal.throwIfAborted();
    if (!video.videoWidth || !video.videoHeight)
      throw new Error("No preview frame");
    const canvas = document.createElement("canvas");
    canvas.width = PREVIEW_MAX_WIDTH;
    canvas.height = PREVIEW_MAX_HEIGHT;
    const context = canvas.getContext("2d", { alpha: false });
    if (!context) throw new Error("Preview unavailable");
    context.fillStyle = "black";
    context.fillRect(0, 0, canvas.width, canvas.height);
    const scale = Math.min(
      canvas.width / video.videoWidth,
      canvas.height / video.videoHeight,
    );
    const width = video.videoWidth * scale;
    const height = video.videoHeight * scale;
    context.drawImage(
      video,
      (canvas.width - width) / 2,
      (canvas.height - height) / 2,
      width,
      height,
    );
    const image = await new Promise<Blob>((resolve, reject) =>
      canvas.toBlob(
        (value) =>
          value ? resolve(value) : reject(new Error("Preview unavailable")),
        "image/jpeg",
        0.6,
      ),
    );
    signal.throwIfAborted();
    if (image.size > PREVIEW_MAX_BYTES) throw new Error("Preview too large");
    return image;
  } finally {
    video.pause();
    video.srcObject = null;
    video.remove();
  }
}
