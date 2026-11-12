import { useEffect, useId, useRef, useState } from "react";
import type { FormEvent } from "react";
import type { UsagePoint, UsageReport } from "@freedomstream/contracts";
import { ApiError, request } from "./api";
import { dataSize, periodRange, speed, type Period } from "./usage";
import "./admin.css";
import { AdminRooms } from "./AdminRooms";

const periods: Period[] = [
  "Real time",
  "Today",
  "7 days",
  "Month",
  "Custom period",
];
const dateInput = (date: Date) =>
  `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
const errorText = (e: unknown) =>
  e instanceof Error ? e.message : "Could not connect. Please try again.";

export function Admin() {
  const [auth, setAuth] = useState<"loading" | "login" | "ready" | "error">(
    "loading",
  );
  const [error, setError] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [busy, setBusy] = useState(false);
  const [section, setSection] = useState<"usage" | "rooms">("usage");
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    const abort = new AbortController();
    request("/api/admin/session", { signal: abort.signal })
      .then(() => setAuth("ready"))
      .catch((e: unknown) => {
        if (abort.signal.aborted) return;
        if (e instanceof ApiError && e.status === 401) setAuth("login");
        else {
          setAuth("error");
          setError(errorText(e));
        }
      });
    return () => abort.abort();
  }, [retry]);
  const login = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError("");
    try {
      await request("/api/admin/login", {
        method: "POST",
        body: JSON.stringify({ password }),
      });
      setPassword("");
      setAuth("ready");
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  const logout = async () => {
    setBusy(true);
    setError("");
    try {
      await request("/api/admin/logout", { method: "POST" });
      setAuth("login");
      setSection("usage");
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="admin-shell">
      <header className="admin-header">
        <a href="/" className="admin-brand">
          FreedomStream
        </a>
        {auth === "ready" && (
          <div>
            <span>Administrator</span>
            <button onClick={() => void logout()} disabled={busy}>
              Sign out
            </button>
          </div>
        )}
      </header>
      {auth === "loading" ? (
        <div className="admin-center" role="status">
          Checking administrator access…
        </div>
      ) : auth === "ready" ? (
        <>
          <div className="admin-top-error">
            {error && <p role="alert">{error}</p>}
          </div>
          <nav className="admin-nav" aria-label="Administration sections">
            <button
              aria-pressed={section === "usage"}
              onClick={() => setSection("usage")}
            >
              Usage
            </button>
            <button
              aria-pressed={section === "rooms"}
              onClick={() => setSection("rooms")}
            >
              Rooms
            </button>
          </nav>
          {section === "usage" ? (
            <Dashboard
              onExpired={() => {
                setAuth("login");
                setError("Your session expired. Sign in again.");
              }}
            />
          ) : (
            <AdminRooms
              onExpired={() => {
                setAuth("login");
                setError("Your session expired. Sign in again.");
              }}
            />
          )}
        </>
      ) : (
        <div className="admin-center">
          <div className="admin-login-wrap">
            {auth === "error" ? (
              <section className="admin-panel">
                <h1>Could not connect</h1>
                <p role="alert">{error}</p>
                <button
                  className="admin-primary"
                  onClick={() => {
                    setAuth("loading");
                    setError("");
                    setRetry(retry + 1);
                  }}
                >
                  Try again
                </button>
              </section>
            ) : (
              <form
                className="admin-panel admin-login"
                onSubmit={(event) => void login(event)}
              >
                <svg
                  className="admin-lock"
                  width="40"
                  height="44"
                  viewBox="0 0 40 44"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2.5"
                  aria-hidden="true"
                >
                  <rect x="5" y="19" width="30" height="22" rx="4" />
                  <path d="M11 19V12a9 9 0 0 1 18 0v7M20 28v6" />
                </svg>
                <h1>Administrator login</h1>
                <p>Sign in to manage rooms and view service usage.</p>
                <label className="admin-account">
                  Account
                  <input
                    value="Administrator"
                    readOnly
                    autoComplete="username"
                    name="username"
                  />
                </label>
                <label htmlFor="admin-password">Password</label>
                <div className="admin-password">
                  <input
                    id="admin-password"
                    name="password"
                    type={showPassword ? "text" : "password"}
                    autoComplete="current-password"
                    required
                    maxLength={256}
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                  />
                  <button
                    type="button"
                    aria-label={
                      showPassword ? "Hide password" : "Show password"
                    }
                    onClick={() => setShowPassword(!showPassword)}
                  >
                    {showPassword ? "Hide" : "Show"}
                  </button>
                </div>
                {error && (
                  <p className="admin-error" role="alert">
                    {error}
                  </p>
                )}
                <button className="admin-primary" disabled={busy}>
                  {busy ? "Signing in…" : "Sign in"}
                </button>
              </form>
            )}
            <p className="admin-private">Private administrator access</p>
          </div>
        </div>
      )}
    </div>
  );
}

function Dashboard({ onExpired }: { onExpired: () => void }) {
  const [period, setPeriod] = useState<Period>("Today");
  const [custom, setCustom] = useState<[number, number] | null>(null);
  const [from, setFrom] = useState(dateInput(new Date()));
  const [to, setTo] = useState(dateInput(new Date()));
  const [data, setData] = useState<UsageReport | null>(null);
  const [error, setError] = useState("");
  const [validation, setValidation] = useState("");
  const [page, setPage] = useState(0);
  const [refresh, setRefresh] = useState(0);
  useEffect(() => {
    const abort = new AbortController();
    let pending = false;
    const load = async () => {
      if (pending || document.hidden) return;
      pending = true;
      const range = period === "Custom period" ? custom : periodRange(period);
      if (!range) {
        pending = false;
        return;
      }
      try {
        const report = await request<UsageReport>(
          `/api/admin/usage?from=${range[0]}&to=${range[1]}&page=${page}`,
          {
            signal: AbortSignal.any([abort.signal, AbortSignal.timeout(10000)]),
          },
        );
        if (!abort.signal.aborted) {
          setData(report);
          setError("");
        }
      } catch (e) {
        if (!abort.signal.aborted) {
          if (e instanceof ApiError && e.status === 401) onExpired();
          else setError(errorText(e));
        }
      } finally {
        pending = false;
      }
    };
    setData(null);
    void load();
    const timer = setInterval(() => void load(), 5000);
    const visible = () => {
      if (!document.hidden) void load();
    };
    document.addEventListener("visibilitychange", visible);
    return () => {
      abort.abort();
      clearInterval(timer);
      document.removeEventListener("visibilitychange", visible);
    };
    // onExpired only transitions authentication; do not restart polling on renders.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [period, custom, page, refresh]);
  const apply = (event: FormEvent) => {
    event.preventDefault();
    const start = new Date(`${from}T00:00:00`);
    const end = new Date(`${to}T00:00:00`);
    end.setDate(end.getDate() + 1);
    if (
      !Number.isFinite(start.getTime()) ||
      !Number.isFinite(end.getTime()) ||
      end <= start ||
      end.getTime() - start.getTime() > 366 * 86400000 ||
      start > new Date() ||
      end.getTime() > Date.now() + 86400000
    ) {
      setValidation("Choose a past or current period of up to 366 days.");
      return;
    }
    setValidation("");
    setPage(0);
    setCustom([start.getTime(), end.getTime()]);
  };
  return (
    <main className="admin-main">
      <div className="admin-heading">
        <div>
          <h1>Usage</h1>
          <p>Service media traffic overview</p>
        </div>
        {data && (
          <span className={`admin-status ${data.available ? "is-live" : ""}`}>
            {data.available ? "Collecting live" : "Collection unavailable"}
          </span>
        )}
      </div>
      <div className="admin-filter-row">
        <div className="admin-periods" role="group" aria-label="Usage period">
          {periods.map((p) => (
            <button
              key={p}
              aria-pressed={p === period}
              onClick={() => {
                setPeriod(p);
                setPage(0);
                setValidation("");
              }}
            >
              {p}
            </button>
          ))}
        </div>
        <span className="admin-timezone">
          {Intl.DateTimeFormat().resolvedOptions().timeZone}
        </span>
      </div>
      {period === "Custom period" && (
        <form className="admin-custom" onSubmit={apply}>
          <label>
            From
            <input
              type="date"
              required
              value={from}
              max={dateInput(new Date())}
              onChange={(e) => setFrom(e.target.value)}
            />
          </label>
          <label>
            Through
            <input
              type="date"
              required
              value={to}
              min={from}
              max={dateInput(new Date())}
              onChange={(e) => setTo(e.target.value)}
            />
          </label>
          <button className="admin-primary">Apply period</button>
          {validation && <p role="alert">{validation}</p>}
        </form>
      )}
      {error && (
        <div className="admin-notice" role="alert">
          {error} {data && "Displayed figures may be out of date."}
          <button onClick={() => setRefresh(refresh + 1)}>Retry</button>
        </div>
      )}
      {!data ? (
        <div className="admin-empty" role="status">
          {period === "Custom period" && !custom
            ? "Select dates and apply the period."
            : error
              ? "Usage could not be loaded."
              : "Loading usage…"}
        </div>
      ) : (
        <>
          {!data.available && (
            <p className="admin-notice">
              Live collection is unavailable. Stored history remains visible;
              missing measurements are gaps, not zero traffic.
            </p>
          )}
          <div className="admin-summary">
            <Metric
              label="Uploaded to server"
              bytes={data.uploadedBytes}
              hint="Server inbound"
            />
            <Metric
              label="Downloaded from server"
              bytes={data.downloadedBytes}
              hint="Server outbound"
            />
            <Metric
              label="Total transfer"
              bytes={data.uploadedBytes + data.downloadedBytes}
              hint="Selected period"
            />
          </div>
          <section className="admin-panel admin-server-chart">
            <div className="admin-section-heading">
              <h2>Server traffic over time</h2>
              <Legend />
            </div>
            <TrafficChart
              points={data.points}
              bucketMs={data.bucketMs}
              label="Server traffic"
            />
          </section>
          <section className="admin-users">
            <div className="admin-section-heading">
              <div>
                <h2>Usage by user</h2>
                <p>When each user was active and how much they transferred</p>
              </div>
              <span>
                {data.userCount} {data.userCount === 1 ? "session" : "sessions"}
              </span>
            </div>
            {data.users.length === 0 ? (
              <div className="admin-panel admin-empty">
                No measured user traffic in this period.
              </div>
            ) : (
              <div className="admin-user-grid">
                {data.users.map((user) => (
                  <article key={user.id} className="admin-panel admin-user">
                    <div className="admin-user-heading">
                      <h3>{user.displayName}</h3>
                      <span title={`Room ${user.roomId} · Session ${user.id}`}>
                        Session {user.id.slice(0, 8)}
                      </span>
                    </div>
                    <div className="admin-user-totals">
                      <Metric label="Uploaded" bytes={user.uploadedBytes} />
                      <Metric label="Downloaded" bytes={user.downloadedBytes} />
                      <Metric
                        label="Total"
                        bytes={user.uploadedBytes + user.downloadedBytes}
                      />
                    </div>
                    <Legend />
                    <TrafficChart
                      points={user.points}
                      bucketMs={data.bucketMs}
                      label={`${user.displayName} traffic`}
                      compact
                    />
                  </article>
                ))}
              </div>
            )}
            {data.userCount > 24 && (
              <nav className="admin-pagination" aria-label="User pages">
                <button disabled={page === 0} onClick={() => setPage(page - 1)}>
                  Previous
                </button>
                <span>
                  Page {page + 1} of {Math.ceil(data.userCount / 24)}
                </span>
                <button
                  disabled={(page + 1) * 24 >= data.userCount}
                  onClick={() => setPage(page + 1)}
                >
                  Next
                </button>
              </nav>
            )}
          </section>
          <footer className="admin-footer">
            <p>Upload = user → server · Download = server → user</p>
            <p>Speed in Mb/s · Data in MB or GB</p>
            <p>
              Users are room sessions; returning with a new session creates a
              separate entry. Media counters include retransmissions; HTTPS and
              relay overhead are excluded. History is retained for{" "}
              {data.retentionDays} days.
            </p>
            {data.lastSampleAt && (
              <p>
                Last measurement: {new Date(data.lastSampleAt).toLocaleString()}
              </p>
            )}
          </footer>
        </>
      )}
    </main>
  );
}
function Metric({
  label,
  bytes,
  hint,
}: {
  label: string;
  bytes: number;
  hint?: string;
}) {
  return (
    <div className="admin-metric">
      <span>{label}</span>
      <strong>{dataSize(bytes)}</strong>
      {hint && <small>{hint}</small>}
    </div>
  );
}
function Legend() {
  return (
    <div className="admin-legend">
      <span>
        <i />
        To server
      </span>
      <span>
        <i />
        From server
      </span>
    </div>
  );
}
function TrafficChart({
  points,
  bucketMs,
  label,
  compact = false,
}: {
  points: UsagePoint[];
  bucketMs: number;
  label: string;
  compact?: boolean;
}) {
  const id = useId();
  const [hover, setHover] = useState<number | null>(null);
  const svg = useRef<SVGSVGElement>(null);
  const [narrow, setNarrow] = useState(compact);
  useEffect(() => {
    const element = svg.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) =>
      setNarrow(compact || entry!.contentRect.width < 600),
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [compact]);
  const width = narrow ? 400 : 1000,
    height = narrow ? 180 : 225,
    left = 42,
    right = 12,
    top = 25,
    bottom = 30;
  const usable = width - left - right,
    plotHeight = height - top - bottom;
  const rates = points.map((p) => ({
    up: speed(p.uploadedBytes, p.durationMs),
    down: speed(p.downloadedBytes, p.durationMs),
  }));
  const maximum = Math.max(1, ...rates.flatMap((p) => [p.up, p.down]));
  const max =
    Math.ceil(maximum / Math.pow(10, Math.floor(Math.log10(maximum)))) *
    Math.pow(10, Math.floor(Math.log10(maximum)));
  const x = (i: number) =>
    left +
    (points.length <= 1 ? usable / 2 : (i / (points.length - 1)) * usable);
  const y = (n: number) => top + plotHeight * (1 - n / max);
  const path = (key: "up" | "down") => {
    let drawing = false;
    return points
      .map((p, i) => {
        if (!p.durationMs) {
          drawing = false;
          return "";
        }
        const command = drawing ? "L" : "M";
        drawing = true;
        return `${command}${x(i).toFixed(2)},${y(rates[i]![key]).toFixed(2)}`;
      })
      .join(" ");
  };
  const time = (at: number) =>
    new Intl.DateTimeFormat(undefined, {
      ...((points.at(-1)?.at ?? 0) - (points[0]?.at ?? 0) >= 86400000
        ? { month: "short", day: "numeric" }
        : {}),
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    }).format(at);
  const selected = hover === null ? undefined : points[hover];
  return (
    <div className="admin-chart">
      <svg
        ref={svg}
        viewBox={`0 0 ${width} ${height}`}
        role="img"
        aria-labelledby={id}
        onPointerLeave={() => setHover(null)}
        onPointerMove={(event) => {
          const rect = event.currentTarget.getBoundingClientRect();
          setHover(
            Math.max(
              0,
              Math.min(
                points.length - 1,
                Math.round(
                  ((((event.clientX - rect.left) / rect.width) * width - left) /
                    usable) *
                    (points.length - 1),
                ),
              ),
            ),
          );
        }}
      >
        <title id={id}>
          {label}. Upload and download speeds in megabits per second. Missing
          measurements appear as gaps.
        </title>
        <text x="8" y="13" className="admin-axis-unit">
          Mb/s
        </text>
        {[0, 0.5, 1].map((f) => (
          <g key={f}>
            <line
              x1={left}
              x2={width - right}
              y1={y(max * f)}
              y2={y(max * f)}
              className="admin-gridline"
            />
            <text x={left - 8} y={y(max * f) + 4} textAnchor="end">
              {(max * f).toLocaleString(undefined, {
                maximumFractionDigits: 2,
              })}
            </text>
          </g>
        ))}
        <path d={path("up")} className="admin-line admin-up" />
        <path d={path("down")} className="admin-line admin-down" />
        {points.flatMap((point, i) =>
          point.durationMs &&
          (!points[i - 1]?.durationMs || !points[i + 1]?.durationMs)
            ? [
                <circle
                  key={`up-${i}`}
                  cx={x(i)}
                  cy={y(rates[i]!.up)}
                  r={3}
                  className="admin-up-dot"
                />,
                <circle
                  key={`down-${i}`}
                  cx={x(i)}
                  cy={y(rates[i]!.down)}
                  r={3}
                  className="admin-down-dot"
                />,
              ]
            : [],
        )}
        {[0, Math.floor((points.length - 1) / 2), points.length - 1]
          .filter((n, i, a) => a.indexOf(n) === i)
          .map(
            (i) =>
              points[i] && (
                <text
                  key={i}
                  x={x(i)}
                  y={height - 7}
                  textAnchor={
                    i === 0
                      ? "start"
                      : i === points.length - 1
                        ? "end"
                        : "middle"
                  }
                >
                  {time(points[i]!.at)}
                </text>
              ),
          )}
        {hover !== null && selected && (
          <line
            x1={x(hover)}
            x2={x(hover)}
            y1={top}
            y2={top + plotHeight}
            className="admin-crosshair"
          />
        )}
      </svg>
      <div className="admin-chart-detail" aria-live="off">
        {selected
          ? `${time(selected.at)} · ${selected.durationMs ? `↑ ${speed(selected.uploadedBytes, selected.durationMs).toFixed(2)} Mb/s · ↓ ${speed(selected.downloadedBytes, selected.durationMs).toFixed(2)} Mb/s · ${dataSize(selected.uploadedBytes + selected.downloadedBytes)}` : "No measurement"}`
          : points.some((p) => p.durationMs)
            ? "Hover over the graph to inspect usage"
            : "No measurements recorded for this period"}
      </div>
    </div>
  );
}
