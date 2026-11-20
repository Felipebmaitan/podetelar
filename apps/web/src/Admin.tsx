import { useEffect, useId, useRef, useState } from "react";
import type { FormEvent } from "react";
import type { UsagePoint, UsageReport } from "@podetelar/contracts";
import { ApiError, request } from "./api";
import { dataSize, periodRange, speed, type Period } from "./usage";
import "./admin.css";
import { AdminRooms } from "./AdminRooms";
import { AdminProfiles } from "./AdminProfiles";
import { AdminDiscord } from "./AdminDiscord";
import { BrandMark } from "./BrandMark";

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

function NavIcon({
  kind,
}: {
  kind: "rooms" | "discord" | "profiles" | "usage" | "signout";
}) {
  const props = {
    width: 16,
    height: 16,
    viewBox: "0 0 16 16",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.5,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    "aria-hidden": true,
  };
  switch (kind) {
    case "rooms":
      return (
        <svg {...props}>
          <rect x="2" y="2.5" width="5" height="5" rx="1.2" />
          <rect x="9" y="2.5" width="5" height="5" rx="1.2" />
          <rect x="2" y="9" width="5" height="4.5" rx="1.2" />
          <rect x="9" y="9" width="5" height="4.5" rx="1.2" />
        </svg>
      );
    case "discord":
      return (
        <svg {...props}>
          <path d="M3 10V8a5 5 0 0 1 10 0v2" />
          <rect x="2" y="9.5" width="2.6" height="4" rx="1" />
          <rect x="11.4" y="9.5" width="2.6" height="4" rx="1" />
        </svg>
      );
    case "profiles":
      return (
        <svg {...props}>
          <circle cx="8" cy="5.5" r="2.75" />
          <path d="M2.75 13.5c.8-2.4 2.75-3.5 5.25-3.5s4.45 1.1 5.25 3.5" />
        </svg>
      );
    case "usage":
      return (
        <svg {...props}>
          <path d="M2.25 11.5 5.5 7.5 8.5 9.5 13.75 4" />
          <path d="M10.75 4h3v3" />
        </svg>
      );
    case "signout":
      return (
        <svg {...props}>
          <path d="M6.5 2.5H3.5a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h3" />
          <path d="M10 11 13.25 8 10 5" />
          <path d="M13 8H6" />
        </svg>
      );
  }
}

export function Admin() {
  const [auth, setAuth] = useState<"loading" | "login" | "ready" | "error">(
    "loading",
  );
  const [error, setError] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [busy, setBusy] = useState(false);
  const [section, setSection] = useState<
    "usage" | "rooms" | "profiles" | "discord"
  >("rooms");
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
      setSection("rooms");
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div
      className={`admin-shell ${auth === "ready" ? "admin-authenticated" : ""}`}
    >
      {auth === "ready" && (
        <header className="admin-header">
          <a href="/" className="admin-brand">
            <BrandMark className="admin-brand-mark" />
            <span>PodeTelar</span>
          </a>
          <div className="admin-rail-footer">
            <button
              className="admin-signout"
              onClick={() => void logout()}
              disabled={busy}
            >
              <NavIcon kind="signout" />
              <span>Sign out</span>
            </button>
          </div>
        </header>
      )}
      {auth === "loading" ? (
        <main className="admin-center">
          <span role="status">Checking administrator access…</span>
        </main>
      ) : auth === "ready" ? (
        <>
          <div className="admin-top-error">
            {error && <p role="alert">{error}</p>}
          </div>
          <nav className="admin-nav" aria-label="Administration sections">
            <button
              aria-pressed={section === "rooms"}
              onClick={() => setSection("rooms")}
            >
              <NavIcon kind="rooms" />
              <span>Rooms</span>
            </button>
            <button
              aria-pressed={section === "discord"}
              onClick={() => setSection("discord")}
            >
              <NavIcon kind="discord" />
              <span>Discord</span>
            </button>
            <button
              aria-pressed={section === "profiles"}
              onClick={() => setSection("profiles")}
            >
              <NavIcon kind="profiles" />
              <span>Guest profiles</span>
            </button>
            <button
              aria-pressed={section === "usage"}
              onClick={() => setSection("usage")}
            >
              <NavIcon kind="usage" />
              <span>Usage</span>
            </button>
          </nav>
          {section === "discord" ? (
            <AdminDiscord
              onExpired={() => {
                setAuth("login");
                setError("Your session expired. Sign in again.");
              }}
            />
          ) : section === "profiles" ? (
            <AdminProfiles
              onExpired={() => {
                setAuth("login");
                setError("Your session expired. Sign in again.");
              }}
            />
          ) : section === "usage" ? (
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
        <main className="admin-center">
          <div className="admin-login-wrap">
            {auth === "error" ? (
              <section className="admin-panel admin-login">
                <div className="admin-login-brand">
                  <BrandMark className="admin-brand-mark" />
                  <span>PodeTelar</span>
                </div>
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
                <div className="admin-login-brand">
                  <BrandMark className="admin-brand-mark" />
                  <span>PodeTelar</span>
                </div>
                <h1>Administrator</h1>
                <input
                  className="sr-only"
                  value="Administrator"
                  readOnly
                  tabIndex={-1}
                  aria-hidden="true"
                  autoComplete="username"
                  name="username"
                />
                <div className="admin-field">
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
          </div>
        </main>
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
  function selectPeriod(next: Period) {
    setPeriod(next);
    setPage(0);
    setValidation("");
  }

  return (
    <main className="admin-main">
      <div className="admin-heading">
        <h1>Usage</h1>
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
              onClick={() => selectPeriod(p)}
            >
              {p}
            </button>
          ))}
        </div>
        <label className="admin-period-select">
          Period
          <select
            value={period}
            onChange={(e) => selectPeriod(e.target.value as Period)}
          >
            {periods.map((p) => (
              <option key={p}>{p}</option>
            ))}
          </select>
        </label>
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
              Live collection is unavailable. Stored history remains visible.
            </p>
          )}
          <div className="admin-summary">
            <Metric label="Uploaded to server" bytes={data.uploadedBytes} />
            <Metric
              label="Downloaded from server"
              bytes={data.downloadedBytes}
            />
            <Metric
              label="Total transfer"
              bytes={data.uploadedBytes + data.downloadedBytes}
            />
          </div>
          <section className="admin-panel admin-server-chart">
            <div className="admin-section-heading">
              <h2>Server traffic</h2>
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
              <h2>Usage by PC</h2>
              <span>
                {data.userCount} {data.userCount === 1 ? "entry" : "entries"}
              </span>
            </div>
            {data.users.length === 0 ? (
              <div className="admin-panel admin-empty">
                No measured PC traffic in this period.
              </div>
            ) : (
              <div className="admin-user-grid">
                {data.users.map((user) => (
                  <article key={user.id} className="admin-panel admin-user">
                    <div className="admin-user-heading">
                      <h3>{user.displayName}</h3>
                      <span title={user.id}>
                        {user.identityKind === "device"
                          ? `PC ${user.id.slice(4, 16)}`
                          : `Session ${user.id.slice(0, 8)}`}
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
                    <details className="admin-session-details">
                      <summary>
                        {user.sessionCount}{" "}
                        {user.sessionCount === 1 ? "session" : "sessions"}
                      </summary>

                      {user.sessions.length < user.sessionCount && (
                        <p>
                          Latest {user.sessions.length} of {user.sessionCount}{" "}
                          sessions.
                        </p>
                      )}
                      <ul>
                        {user.sessions.map((session) => (
                          <li key={session.id}>
                            <strong>{session.displayName}</strong>
                            <span>
                              {session.admittedAt
                                ? new Date(session.admittedAt).toLocaleString()
                                : "Admission time unavailable"}
                            </span>
                            <span>
                              ↑ {dataSize(session.uploadedBytes)} · ↓{" "}
                              {dataSize(session.downloadedBytes)}
                            </span>
                          </li>
                        ))}
                      </ul>
                      <details className="admin-usage-identifiers">
                        <summary>Technical identifiers</summary>
                        <p className="admin-device-id">
                          {user.identityKind === "device"
                            ? "PC ID"
                            : "Session ID"}
                          : <code>{user.id}</code>
                        </p>
                        <ul>
                          {user.sessions.map((session) => (
                            <li key={session.id}>
                              <strong>{session.displayName}</strong>
                              <span>
                                Room ID: <code>{session.roomId}</code>
                              </span>
                              <span>
                                Session ID: <code>{session.id}</code>
                              </span>
                            </li>
                          ))}
                        </ul>
                      </details>
                    </details>
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
            <p>Usage history is kept for {data.retentionDays} days.</p>
            {data.lastSampleAt && (
              <p>
                Last sample {new Date(data.lastSampleAt).toLocaleTimeString()}
              </p>
            )}
          </footer>
        </>
      )}
    </main>
  );
}

function Metric({ label, bytes }: { label: string; bytes: number }) {
  return (
    <div className="admin-metric">
      <span>{label}</span>
      <strong>{dataSize(bytes)}</strong>
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
  const [width, setWidth] = useState(compact ? 400 : 1000);
  useEffect(() => {
    const element = svg.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) =>
      setWidth(Math.max(1, Math.round(entry!.contentRect.width))),
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [compact]);
  const narrow = compact || width < 600;
  const height = narrow ? 180 : 225,
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
  const area = (key: "up" | "down") => {
    const baseline = top + plotHeight;
    let start: number | null = null;
    let last = 0;
    const segments: string[] = [];
    let current = "";
    points.forEach((p, i) => {
      if (!p.durationMs) {
        if (start !== null) {
          segments.push(
            `${current} L${x(last).toFixed(2)},${baseline.toFixed(2)} L${x(start).toFixed(2)},${baseline.toFixed(2)} Z`,
          );
          start = null;
          current = "";
        }
        return;
      }
      if (start === null) {
        start = i;
        current = `M${x(i).toFixed(2)},${y(rates[i]![key]).toFixed(2)}`;
      } else {
        current += ` L${x(i).toFixed(2)},${y(rates[i]![key]).toFixed(2)}`;
      }
      last = i;
    });
    if (start !== null) {
      segments.push(
        `${current} L${x(last).toFixed(2)},${baseline.toFixed(2)} L${x(start).toFixed(2)},${baseline.toFixed(2)} Z`,
      );
    }
    return segments.join(" ");
  };
  const crossesMidnight =
    points.length > 0 &&
    new Date(points[0]!.at).toDateString() !==
      new Date(points[points.length - 1]!.at).toDateString();
  const time = (at: number) =>
    new Intl.DateTimeFormat(undefined, {
      ...(crossesMidnight || bucketMs >= 86400000
        ? { month: "short", day: "numeric" }
        : {}),
      ...(bucketMs < 86400000
        ? {
            hour: "2-digit",
            minute: "2-digit",
            ...(bucketMs < 60000 ? { second: "2-digit" } : {}),
          }
        : {}),
    }).format(at);
  const ticks = [0, 0.25, 0.5, 0.75, 1];
  const labels = points.length
    ? [
        0,
        ...(narrow
          ? usable >= 195 && !crossesMidnight && bucketMs >= 60000
            ? [Math.floor((points.length - 1) / 2)]
            : []
          : [
              Math.floor((points.length - 1) / 3),
              Math.floor(((points.length - 1) * 2) / 3),
            ]),
        points.length - 1,
      ].filter((v, i, a) => a.indexOf(v) === i)
    : [];
  const active = hover === null ? null : points[hover];
  const select = (clientX: number) => {
    const rect = svg.current?.getBoundingClientRect();
    if (!rect || !points.length) return;
    const ratio = (clientX - rect.left) / rect.width;
    const index = Math.round(
      ((ratio * width - left) / usable) * (points.length - 1),
    );
    setHover(Math.max(0, Math.min(points.length - 1, index)));
  };
  const upGradId = `${id}-up`;
  const downGradId = `${id}-down`;
  return (
    <div className="admin-chart">
      <svg
        ref={svg}
        viewBox={`0 0 ${width} ${height}`}
        role="img"
        aria-labelledby={id}
        aria-describedby={`${id}-detail`}
        tabIndex={points.length ? 0 : undefined}
        onFocus={() => setHover(0)}
        onBlur={() => setHover(null)}
        onKeyDown={(event) => {
          if (!points.length) return;
          const index = hover ?? 0;
          const last = points.length - 1;
          const next = {
            ArrowLeft: Math.max(0, index - 1),
            ArrowRight: Math.min(last, index + 1),
            Home: 0,
            End: last,
          }[event.key];
          if (next === undefined) return;
          event.preventDefault();
          setHover(next);
        }}
        onMouseMove={(e) => select(e.clientX)}
        onMouseLeave={() => setHover(null)}
        onTouchStart={(e) => e.touches[0] && select(e.touches[0].clientX)}
        onTouchMove={(e) => e.touches[0] && select(e.touches[0].clientX)}
      >
        <title id={id}>{label} in megabits per second</title>
        <defs>
          <linearGradient id={upGradId} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#E5A93C" stopOpacity="0.22" />
            <stop offset="100%" stopColor="#E5A93C" stopOpacity="0" />
          </linearGradient>
          <linearGradient id={downGradId} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#7AA7D9" stopOpacity="0.18" />
            <stop offset="100%" stopColor="#7AA7D9" stopOpacity="0" />
          </linearGradient>
        </defs>
        <text className="admin-axis-unit" x={left} y={13}>
          Mb/s
        </text>
        {ticks.map((tick) => {
          const value = max * tick;
          const pos = y(value);
          return (
            <g key={tick}>
              <line
                className="admin-gridline"
                x1={left}
                x2={width - right}
                y1={pos}
                y2={pos}
              />
              <text x={left - 8} y={pos + 4} textAnchor="end">
                {value >= 10
                  ? Math.round(value)
                  : value.toFixed(1).replace(/\.0$/, "")}
              </text>
            </g>
          );
        })}
        {labels.map((index) => (
          <text
            key={index}
            x={x(index)}
            y={height - 8}
            textAnchor={
              index === 0
                ? "start"
                : index === points.length - 1
                  ? "end"
                  : "middle"
            }
          >
            {time(points[index]!.at)}
          </text>
        ))}
        <path d={area("down")} fill={`url(#${downGradId})`} />
        <path d={area("up")} fill={`url(#${upGradId})`} />
        <path className="admin-line admin-down" d={path("down")} />
        <path className="admin-line admin-up" d={path("up")} />
        {active && active.durationMs > 0 && hover !== null && (
          <>
            <line
              className="admin-crosshair"
              x1={x(hover)}
              x2={x(hover)}
              y1={top}
              y2={top + plotHeight}
            />
            <circle
              className="admin-down-dot"
              cx={x(hover)}
              cy={y(rates[hover]!.down)}
              r="3.5"
            />
            <circle
              className="admin-up-dot"
              cx={x(hover)}
              cy={y(rates[hover]!.up)}
              r="3.5"
            />
          </>
        )}
      </svg>
      <p id={`${id}-detail`} className="admin-chart-detail">
        {active && hover !== null
          ? active.durationMs
            ? `${time(active.at)} · To server ${rates[hover]!.up.toFixed(2)} Mb/s (${dataSize(active.uploadedBytes)}) · From server ${rates[hover]!.down.toFixed(2)} Mb/s (${dataSize(active.downloadedBytes)})`
            : `${time(active.at)} · No measurement`
          : ""}
      </p>
    </div>
  );
}
