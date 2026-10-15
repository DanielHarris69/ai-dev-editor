import { useEffect, useState } from "react";
import { toast } from "sonner";
import { ListProviders, RemoveProvider, SaveProvider } from "../../bridge/commands";
import { BTN_SECONDARY } from "./ui/buttons";

export interface ProviderRecord {
  id: string;
  label: string;
  kind: string;
  enabled: boolean;
  modelAllowlist: string[];
  hasCredentials: boolean;
}

const KIND_LABELS: Record<string, string> = {
  reported: "Other provider (reported usage)",
  hermes: "Hermes (reports usage)",
  vertex: "Google Vertex AI",
  devpass: "DevPass",
};

const CREDENTIAL_HINT: Record<string, string> = {
  reported: "No provider credential needed. Your integration reports usage using this identifier.",
  hermes: "Not needed: Hermes posts usage to the local proxy.",
  vertex: "Service-account JSON path, or leave blank for a future application-default credentials connector.",
  devpass: "DevPass API token.",
};

const BLANK: ProviderRecord = {
  id: "",
  label: "",
  kind: "vertex",
  enabled: true,
  modelAllowlist: [],
  hasCredentials: false,
};

export function ProviderSettings() {
  const [providers, setProviders] = useState<ProviderRecord[]>([]);
  const [draft, setDraft] = useState<ProviderRecord | null>(null);
  const [isNew, setIsNew] = useState(false);
  const [credential, setCredential] = useState("");
  const [models, setModels] = useState("");
  const [busy, setBusy] = useState(false);

  const load = async () => {
    try {
      const list = (await ListProviders()) as ProviderRecord[];
      setProviders(Array.isArray(list) ? list : []);
    } catch (err) {
      toast.error(String(err));
    }
  };

  useEffect(() => {
    void load();
  }, []);

  const open = (p: ProviderRecord | null) => {
    setIsNew(p === null);
    setDraft(p ?? { ...BLANK });
    setCredential("");
    setModels((p?.modelAllowlist ?? []).join("\n"));
  };

  const save = async () => {
    if (!draft) return;
    setBusy(true);
    try {
      await SaveProvider(
        {
          id: draft.id,
          label: draft.label,
          kind: draft.kind,
          enabled: draft.enabled,
          modelAllowlist: models.split("\n").map((m) => m.trim()).filter(Boolean),
        },
        credential.trim() ? credential.trim() : null,
      );
      setDraft(null);
      setCredential("");
      await load();
      toast.success("Provider saved");
    } catch (err) {
      toast.error(String(err));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (id: string) => {
    setBusy(true);
    try {
      await RemoveProvider(id);
      if (draft?.id === id) setDraft(null);
      await load();
    } catch (err) {
      toast.error(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-3 px-4 py-3" data-settings-row="ai.providers">
      <p className="text-[11px] text-[var(--text-muted)]">
        Providers appear as cards in the spend dashboard. Credentials are stored in the
        system credential store and are never shown again after saving. Vertex and DevPass
        automatic usage collection is not connected yet.
      </p>

      {providers.length === 0 && (
        <p className="text-xs text-[var(--text-muted)]">No providers configured yet.</p>
      )}

      {providers.map((p) => (
        <div
          key={p.id}
          className="flex items-center justify-between gap-3 rounded-md border border-[var(--border)] px-3 py-2"
        >
          <div className="min-w-0">
            <p className="truncate text-sm text-[var(--text-primary)]">{p.label}</p>
            <p className="text-[11px] text-[var(--text-muted)]">
              {KIND_LABELS[p.kind] ?? p.kind} · {p.enabled ? "enabled" : "disabled"} ·{" "}
              {p.hasCredentials ? "credentials saved" : "no credentials"}
            </p>
          </div>
          <div className="flex shrink-0 gap-2">
            <button className={BTN_SECONDARY} onClick={() => open(p)} disabled={busy}>
              Edit
            </button>
            <button className={BTN_SECONDARY} onClick={() => remove(p.id)} disabled={busy}>
              Remove
            </button>
          </div>
        </div>
      ))}

      {!draft && (
        <button className={BTN_SECONDARY} onClick={() => open(null)}>
          Add provider
        </button>
      )}

      {draft && (
        <div className="flex flex-col gap-2 rounded-md border border-[var(--border)] p-3">
          <label className="flex flex-col gap-1 text-xs text-[var(--text-secondary)]">
            Identifier
            <input
              className="rounded-md border border-[var(--border)] bg-transparent px-2 py-1 text-sm"
              value={draft.id}
              disabled={!isNew}
              placeholder="vertex-main"
              onChange={(e) => setDraft({ ...draft, id: e.target.value })}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs text-[var(--text-secondary)]">
            Label
            <input
              className="rounded-md border border-[var(--border)] bg-transparent px-2 py-1 text-sm"
              value={draft.label}
              onChange={(e) => setDraft({ ...draft, label: e.target.value })}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs text-[var(--text-secondary)]">
            Type
            <select
              className="rounded-md border border-[var(--border)] bg-transparent px-2 py-1 text-sm"
              value={draft.kind}
              disabled={!isNew}
              onChange={(e) => setDraft({ ...draft, kind: e.target.value })}
            >
              {Object.entries(KIND_LABELS).map(([k, v]) => (
                <option key={k} value={k}>
                  {v}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs text-[var(--text-secondary)]">
            Models (one per line; empty = all)
            <textarea
              className="min-h-16 rounded-md border border-[var(--border)] bg-transparent px-2 py-1 font-mono text-xs"
              value={models}
              onChange={(e) => setModels(e.target.value)}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs text-[var(--text-secondary)]">
            Credential {draft.hasCredentials && !isNew ? "(saved; enter to replace)" : ""}
            <input
              type="password"
              autoComplete="off"
              className="rounded-md border border-[var(--border)] bg-transparent px-2 py-1 text-sm"
              value={credential}
              onChange={(e) => setCredential(e.target.value)}
            />
            <span className="text-[11px] text-[var(--text-muted)]">
              {CREDENTIAL_HINT[draft.kind] ?? ""}
            </span>
          </label>
          <label className="flex items-center gap-2 text-xs text-[var(--text-secondary)]">
            <input
              type="checkbox"
              checked={draft.enabled}
              onChange={(e) => setDraft({ ...draft, enabled: e.target.checked })}
            />
            Enable usage ingestion
          </label>
          <div className="flex gap-2">
            <button className={BTN_SECONDARY} onClick={save} disabled={busy}>
              Save
            </button>
            <button className={BTN_SECONDARY} onClick={() => setDraft(null)} disabled={busy}>
              Cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
