import { useEffect, useState } from "react";
import { pickProvider, useAgentLimits } from "../hooks/useAgentLimits";
import { UsageProviderCard } from "./UsageProviderCard";
export function CodexSubscription() {
  const { limits, error } = useAgentLimits();
  const codex = pickProvider(limits, "codex");
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 30000);
    return () => clearInterval(t);
  }, []);
  return (
    <div className="my-4">
      {codex ? (
        <UsageProviderCard
          data={codex}
          now={now}
          title="Codex subscription usage"
          footer="Subscription limits across your account. No per-token API charge is assigned. Missing windows are not reported by Codex."
        />
      ) : (
        <p className="rounded-xl border border-[var(--border)] p-4 text-sm text-[var(--text-muted)]">
          Codex subscription usage: waiting for a local Codex usage report.
        </p>
      )}
      {error && (
        <p role="alert" className="text-xs text-[var(--accent-red)]">
          {error}
        </p>
      )}
    </div>
  );
}
