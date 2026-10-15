import { useEffect, useState } from "react";
import { ListProviders, ProviderReadiness } from "../../bridge/commands";

export interface ProviderStatus {
  id: string;
  readiness: string;
  credentialHint: string;
  env: string[];
}

const READINESS_TEXT: Record<string, string> = {
  reported: "Reports usage to the proxy",
  needsCredentials: "Add credentials in Settings → AI & Integrations",
  disabled: "Disabled; new usage is not accepted",
  notConnected: "Credentials saved; live usage not yet connected",
};

export function ProviderStatusStrip() {
  const [statuses, setStatuses] = useState<ProviderStatus[]>([]);
  const [labels, setLabels] = useState<Record<string, string>>({});

  useEffect(() => {
    let alive = true;
    const load = () =>
      ProviderReadiness()
        .then((list: ProviderStatus[] | null) => {
          if (alive) setStatuses(Array.isArray(list) ? list : []);
        })
        .catch(() => {
          if (alive) setStatuses([]);
        });
    ListProviders()
      .then((list: { id: string; label: string }[] | null) => {
        if (!alive || !Array.isArray(list)) return;
        setLabels(Object.fromEntries(list.map((p) => [p.id, p.label])));
      })
      .catch(() => {});
    void load();
    const timer = setInterval(load, 15000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);

  if (statuses.length === 0) return null;

  return (
    <ul className="mb-4 grid gap-2 sm:grid-cols-2" aria-label="Spend providers">
      {statuses.map((s) => (
        <li
          key={s.id}
          className="rounded-lg border border-[var(--border)] px-3 py-2 text-sm"
        >
          <p className="text-[var(--text-primary)]">{labels[s.id] ?? s.id}</p>
          <p className="text-xs text-[var(--text-muted)]">
            {READINESS_TEXT[s.readiness] ?? s.readiness}
          </p>
        </li>
      ))}
    </ul>
  );
}
