import type { ReactNode } from "react";

export function StatusLabel({
  tone = "neutral",
  children,
}: {
  tone?: "good" | "attention" | "neutral";
  children: ReactNode;
}) {
  return (
    <span className={`discord-status discord-status--${tone}`}>{children}</span>
  );
}

export function SectionHeader({
  title,
  description,
  children,
}: {
  title: string;
  description?: string | undefined;
  children?: ReactNode;
}) {
  return (
    <header className="discord-section-header">
      <div>
        <h2>{title}</h2>
        {description && <p>{description}</p>}
      </div>
      {children}
    </header>
  );
}

export function EmptyState({ children }: { children: ReactNode }) {
  return (
    <p className="discord-empty" role="status">
      {children}
    </p>
  );
}

export function ActionGroup({
  title,
  description,
  children,
}: {
  title: string;
  description?: string | undefined;
  children: ReactNode;
}) {
  return (
    <div className="discord-action-group">
      <div>
        <h4>{title}</h4>
        {description && <p>{description}</p>}
      </div>
      <div className="discord-actions">{children}</div>
    </div>
  );
}
