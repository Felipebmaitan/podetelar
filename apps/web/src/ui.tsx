import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ReactNode } from "react";

/** One stroke weight and one grid for every icon in the interface. */
const base = {
  width: 20,
  height: 20,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.75,
  strokeLinecap: "round",
  strokeLinejoin: "round",
  "aria-hidden": true,
  focusable: false,
} as const;

const draw = (children: ReactNode) => (
  <svg {...base} className="icon">
    {children}
  </svg>
);

export const Icon = {
  hash: () => draw(<path d="M5 9h14M5 15h14M10 4 8 20M16 4l-2 16" />),
  usage: () =>
    draw(
      <>
        <path d="M4 20V10M10 20V4M16 20v-8M22 20H2" />
      </>,
    ),
  signOut: () =>
    draw(
      <path d="M9 4H5a1 1 0 0 0-1 1v14a1 1 0 0 0 1 1h4M16 8l4 4-4 4M20 12H9" />,
    ),
  plus: () => draw(<path d="M12 5v14M5 12h14" />),
  search: () =>
    draw(
      <>
        <circle cx="11" cy="11" r="6.5" />
        <path d="m20 20-4.2-4.2" />
      </>,
    ),
  chevron: () => draw(<path d="m9 6 6 6-6 6" />),
  copy: () =>
    draw(
      <>
        <rect x="9" y="9" width="11" height="11" rx="2.5" />
        <path d="M5 15V6a2 2 0 0 1 2-2h9" />
      </>,
    ),
  up: () => draw(<path d="M12 19V5M6 11l6-6 6 6" />),
  down: () => draw(<path d="M12 5v14M6 13l6 6 6-6" />),
  lock: () =>
    draw(
      <>
        <rect x="5" y="11" width="14" height="9" rx="2.5" />
        <path d="M8 11V8a4 4 0 0 1 8 0v3" />
      </>,
    ),
  monitor: () =>
    draw(
      <>
        <rect x="3" y="4" width="18" height="12" rx="2.5" />
        <path d="M8 20h8M12 16v4" />
      </>,
    ),
  users: () =>
    draw(
      <>
        <circle cx="9" cy="8" r="3.2" />
        <path d="M3 20a6 6 0 0 1 12 0M16 5.2a3.2 3.2 0 0 1 0 5.6M18 14.5a6 6 0 0 1 3 5.5" />
      </>,
    ),
  chat: () =>
    draw(
      <path d="M5 5h14a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-7l-5 4v-4H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z" />,
    ),
  check: () => draw(<path d="m5 12.5 4.5 4.5L19 7.5" />),
};

/** Tabular figures that ease toward their next value; settled instantly for reduced motion. */
export function useCountUp(target: number, duration = 700) {
  const [value, setValue] = useState(target);
  const from = useRef(target);
  useEffect(() => {
    const reduced = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;
    if (reduced || from.current === target) {
      from.current = target;
      setValue(target);
      return;
    }
    const start = performance.now();
    const origin = from.current;
    let frame = 0;
    const tick = (now: number) => {
      const t = Math.min(1, (now - start) / duration);
      const eased = 1 - Math.pow(1 - t, 4);
      const next = origin + (target - origin) * eased;
      from.current = next;
      setValue(next);
      if (t < 1) frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [target, duration]);
  return value;
}

/** A segmented control whose active thumb slides between options. */
export function Segmented<T extends string>({
  options,
  value,
  onChange,
  label,
}: {
  options: readonly T[];
  value: T;
  onChange: (value: T) => void;
  label: string;
}) {
  const root = useRef<HTMLDivElement>(null);
  const [thumb, setThumb] = useState<{ x: number; w: number } | null>(null);
  useLayoutEffect(() => {
    const measure = () => {
      const active = root.current?.querySelector<HTMLElement>(
        '[aria-pressed="true"]',
      );
      if (active) setThumb({ x: active.offsetLeft, w: active.offsetWidth });
    };
    measure();
    const observer = new ResizeObserver(measure);
    if (root.current) observer.observe(root.current);
    return () => observer.disconnect();
  }, [value, options]);
  return (
    <div className="segmented" role="group" aria-label={label} ref={root}>
      {thumb && (
        <span
          className="segmented-thumb"
          aria-hidden="true"
          style={{ width: thumb.w, transform: `translateX(${thumb.x}px)` }}
        />
      )}
      {options.map((option) => (
        <button
          key={option}
          type="button"
          aria-pressed={option === value}
          onClick={() => onChange(option)}
        >
          {option}
        </button>
      ))}
    </div>
  );
}

/** Small helper so list entrances can stagger without per-item CSS. */
export const stagger = (index: number) =>
  ({ "--i": Math.min(index, 12) }) as React.CSSProperties;
