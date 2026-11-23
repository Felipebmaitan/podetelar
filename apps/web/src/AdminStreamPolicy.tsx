import { useEffect, useState, type FormEvent } from "react";
import {
  streamFrameRates,
  streamResolutions,
  streamPolicySchema,
  type StreamPolicy,
} from "@podetelar/contracts";
import { ApiError, request } from "./api";

type PolicyResponse = { policy: StreamPolicy; enforcement: boolean };
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
    const policy = streamPolicySchema.safeParse({
      revision: data.policy.revision,
      maxBitrateBps: data.enforcement
        ? bitrate("maxBitrate")
        : data.policy.maxBitrateBps,
      maxResolution: data.enforcement
        ? number("maxResolution")
        : data.policy.maxResolution,
      maxFps: data.enforcement ? number("maxFps") : data.policy.maxFps,
      automaticTargets: streamResolutions.flatMap((resolution) =>
        streamFrameRates.flatMap((fps) => {
          const bitrateBps = bitrate(`target-${resolution}-${fps}`);
          return bitrateBps === null ? [] : [{ resolution, fps, bitrateBps }];
        }),
      ),
    });
    if (!policy.success) {
      setError("Enter bitrates between 1 and 100 Mbps.");
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
            <div className="admin-stream-targets">
              <table>
                <thead>
                  <tr>
                    <th scope="col">Resolution</th>
                    {streamFrameRates.map((fps) => (
                      <th scope="col" key={fps}>
                        {fps} FPS
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {streamResolutions.map((resolution) => (
                    <tr key={resolution}>
                      <th scope="row">{resolutionLabel(resolution)}</th>
                      {streamFrameRates.map((fps) => (
                        <td key={fps}>
                          <input
                            aria-label={`${resolutionLabel(resolution)} at ${fps} FPS, Mbps`}
                            name={`target-${resolution}-${fps}`}
                            type="number"
                            min="1"
                            max="100"
                            step="any"
                            defaultValue={toMbps(
                              policy.automaticTargets.find(
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
