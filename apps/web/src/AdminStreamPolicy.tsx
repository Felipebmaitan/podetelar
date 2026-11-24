import { useEffect, useState, type FormEvent } from "react";
import {
  cameraFrameRates,
  cameraResolutions,
  streamFrameRates,
  streamResolutions,
  streamPolicySchema,
  type StreamPolicy,
} from "@podetelar/contracts";
import { ApiError, request } from "./api";

type PolicyResponse = {
  policy: StreamPolicy;
  enforcement: boolean;
  cameraEnforcement: boolean;
};
const endpoint = "/api/admin/stream-policy";
const resolutionLabel = (resolution: number) =>
  resolution === 2160 ? "4K" : `${resolution}p`;
const toMbps = (bps?: number | null) => (bps == null ? "" : bps / 1_000_000);

export function AdminStreamPolicy({ onExpired }: { onExpired: () => void }) {
  const [data, setData] = useState<PolicyResponse>();
  const [error, setError] = useState("");
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);
  const [loadAttempt, setLoadAttempt] = useState(0);

  useEffect(() => {
    const abort = new AbortController();
    request<PolicyResponse>(endpoint, { signal: abort.signal })
      .then(setData)
      .catch((error: unknown) => {
        if (abort.signal.aborted) return;
        if (error instanceof ApiError && error.status === 401) onExpired();
        else setError("Could not load stream quality settings.");
      });
    return () => abort.abort();
  }, [loadAttempt]);

  const save = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!data || busy) return;
    setSaved(false);
    setError("");
    const fields = new FormData(event.currentTarget);
    const number = (field: string) =>
      fields.get(field) ? Number(fields.get(field)) : null;
    const bitrate = (field: string) => {
      const mbps = number(field);
      return mbps === null ? null : Math.round(mbps * 1_000_000);
    };
    const targets = (
      prefix: string,
      resolutions: readonly number[],
      frameRates: readonly number[],
    ) =>
      resolutions.flatMap((resolution) =>
        frameRates.flatMap((fps) => {
          const bitrateBps = bitrate(`${prefix}-${resolution}-${fps}`);
          return bitrateBps === null ? [] : [{ resolution, fps, bitrateBps }];
        }),
      );
    const maxCameraBitrateBps = data.cameraEnforcement
      ? bitrate("maxCameraBitrate")
      : data.policy.maxCameraBitrateBps;
    const cameraAutomaticTargets = targets(
      "camera-target",
      cameraResolutions,
      cameraFrameRates,
    );
    const policy = streamPolicySchema.safeParse({
      revision: data.policy.revision,
      maxBitrateBps: data.enforcement
        ? bitrate("maxBitrate")
        : data.policy.maxBitrateBps,
      maxResolution: data.enforcement
        ? number("maxResolution")
        : data.policy.maxResolution,
      maxFps: data.enforcement ? number("maxFps") : data.policy.maxFps,
      automaticTargets: targets("target", streamResolutions, streamFrameRates),
      ...(maxCameraBitrateBps != null || data.policy.maxCameraBitrateBps != null
        ? { maxCameraBitrateBps }
        : {}),
      ...(cameraAutomaticTargets.length ||
      data.policy.cameraAutomaticTargets?.length
        ? { cameraAutomaticTargets }
        : {}),
    });
    if (!policy.success) {
      setError(
        "Enter stream bitrates from 1 to 100 Mbps and camera bitrates from 0.5 to 8 Mbps.",
      );
      return;
    }
    setBusy(true);
    try {
      setData(
        await request<PolicyResponse>(endpoint, {
          method: "PUT",
          body: JSON.stringify(policy.data),
        }),
      );
      setSaved(true);
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) onExpired();
      else
        setError(
          error instanceof ApiError
            ? error.message
            : "Could not save stream quality settings.",
        );
    } finally {
      setBusy(false);
    }
  };

  const policy = data?.policy;
  return (
    <main className="admin-main">
      <h1>Stream quality</h1>
      {policy && data ? (
        <form
          className="admin-panel admin-stream-policy"
          key={policy.revision}
          onSubmit={(event) => void save(event)}
        >
          <fieldset disabled={busy || !data.enforcement}>
            <legend>Per-stream limits</legend>
            <p>Tighter limits also apply to streams already live.</p>
            <div className="admin-stream-limits">
              <label>
                Maximum video bitrate (Mbps)
                <input
                  name="maxBitrate"
                  type="number"
                  min="1"
                  max="100"
                  step="any"
                  placeholder="No limit"
                  defaultValue={toMbps(policy.maxBitrateBps)}
                />
              </label>
              <label>
                Maximum resolution
                <select
                  name="maxResolution"
                  defaultValue={policy.maxResolution ?? ""}
                >
                  <option value="">No limit</option>
                  {streamResolutions.map((resolution) => (
                    <option key={resolution} value={resolution}>
                      {resolutionLabel(resolution)}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Maximum frame rate
                <select name="maxFps" defaultValue={policy.maxFps ?? ""}>
                  <option value="">No limit</option>
                  {streamFrameRates.map((fps) => (
                    <option key={fps} value={fps}>
                      {fps} FPS
                    </option>
                  ))}
                </select>
              </label>
            </div>
          </fieldset>
          {!data.enforcement && (
            <p>Stream limits require a policy-enabled media server.</p>
          )}
          <fieldset disabled={busy}>
            <legend>Automatic video bitrate (Mbps)</legend>
            <p>Blank cells use PodeTelar’s built-in recommendation.</p>
            <BitrateTargets
              prefix="target"
              resolutions={streamResolutions}
              frameRates={streamFrameRates}
              targets={policy.automaticTargets}
            />
          </fieldset>
          <fieldset disabled={busy}>
            <legend>Camera</legend>
            <label>
              Maximum camera video bitrate (Mbps)
              <input
                name="maxCameraBitrate"
                type="number"
                min="0.5"
                max="8"
                step="any"
                disabled={!data.cameraEnforcement}
                placeholder="Use the per-stream limit"
                defaultValue={toMbps(policy.maxCameraBitrateBps)}
              />
            </label>
            {!data.cameraEnforcement && (
              <p>
                Camera limits require a media server with camera policy support.
              </p>
            )}
            <p>
              Set Automatic camera bitrates in Mbps. Blank cells use the
              built-in codec-specific targets.
            </p>
            <BitrateTargets
              prefix="camera-target"
              resolutions={cameraResolutions}
              frameRates={cameraFrameRates}
              targets={policy.cameraAutomaticTargets ?? []}
            />
          </fieldset>
          <button className="admin-primary" disabled={busy}>
            {busy ? "Saving…" : "Save stream policy"}
          </button>
        </form>
      ) : error ? null : (
        <p role="status">Loading stream quality settings…</p>
      )}
      {error && <p role="alert">{error}</p>}
      {saved && <p role="status">Stream policy saved.</p>}
      {error && (
        <button
          onClick={() => {
            setError("");
            setSaved(false);
            setLoadAttempt(loadAttempt + 1);
          }}
        >
          Reload settings
        </button>
      )}
    </main>
  );
}

function BitrateTargets({
  prefix,
  resolutions,
  frameRates,
  targets,
}: {
  prefix: "target" | "camera-target";
  resolutions: readonly number[];
  frameRates: readonly number[];
  targets: readonly { resolution: number; fps: number; bitrateBps: number }[];
}) {
  const camera = prefix === "camera-target";
  return (
    <div className="admin-stream-targets">
      <table>
        <thead>
          <tr>
            <th scope="col">Resolution</th>
            {frameRates.map((fps) => (
              <th scope="col" key={fps}>
                {fps} FPS
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {resolutions.map((resolution) => (
            <tr key={resolution}>
              <th scope="row">{resolutionLabel(resolution)}</th>
              {frameRates.map((fps) => (
                <td key={fps}>
                  <input
                    aria-label={`${camera ? "Camera " : ""}${resolutionLabel(resolution)} at ${fps} FPS, Mbps`}
                    name={`${prefix}-${resolution}-${fps}`}
                    type="number"
                    min={camera ? "0.5" : "1"}
                    max={camera ? "8" : "100"}
                    step="any"
                    defaultValue={toMbps(
                      targets.find(
                        (target) =>
                          target.resolution === resolution &&
                          target.fps === fps,
                      )?.bitrateBps,
                    )}
                  />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
