import { CodexSubscription } from "./CodexSubscription";
import { ProviderStatusStrip } from "./ProviderStatusStrip";
import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, Coins, Copy } from "lucide-react";

interface SpendRow {
  provider: string;
  inputTokens: number;
  outputTokens: number;
  cost: number;
  requests: number;
  sessionCount: number;
  firstSeen: string | null;
  lastSeen: string | null;
  reasoningEfforts: string[];
}
interface ProxyInfo {
  url: string;
  token: string;
  status: string;
}
interface SeriesPoint {
  day: string;
  cost: number;
  requests: number;
}
interface ModelSeries {
  model: string;
  reasoningEffort: string | null;
  totalCost: number;
  totalRequests: number;
  points: SeriesPoint[];
}
interface SpendSeries {
  window: string;
  from: string;
  to: string;
  totalCost: number;
  totalRequests: number;
  days: string[];
  series: ModelSeries[];
  tokensByDay: number[];
  costByDay: number[];
}

type WindowKey = "today" | "this_week" | "this_month" | "last_month";

const WINDOWS: { key: WindowKey; label: string }[] = [
  { key: "today", label: "Today" },
  { key: "this_week", label: "This week" },
  { key: "this_month", label: "This month" },
  { key: "last_month", label: "Last month" },
];

const FALLBACK_RATE = 0.79;
const FX_STORAGE_KEY = "astra.spend.gbp_rate";
const FX_DATE_KEY = "astra.spend.gbp_rate_date";

const gbp = (v: number) =>
  new Intl.NumberFormat("en-GB", {
    style: "currency",
    currency: "GBP",
    minimumFractionDigits: 4,
  }).format(v);

const usd = (v: number) =>
  new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 4,
  }).format(v);

interface FxRate {
  rate: number;
  date: string;
  source: "frankfurter" | "fallback";
}

async function loadFxRate(): Promise<FxRate> {
  try {
    const cached = sessionStorage.getItem(FX_STORAGE_KEY);
    const cachedDate = sessionStorage.getItem(FX_DATE_KEY);
    if (cached && cachedDate) {
      return { rate: Number(cached), date: cachedDate, source: "frankfurter" };
    }
    const res = await fetch(
      "https://api.frankfurter.dev/v1/latest?from=USD&to=GBP",
    );
    if (!res.ok) throw new Error(`fx ${res.status}`);
    const json: {
      date: string;
      rates: { GBP: number };
    } = await res.json();
    const rate = json.rates.GBP;
    sessionStorage.setItem(FX_STORAGE_KEY, String(rate));
    sessionStorage.setItem(FX_DATE_KEY, json.date);
    return { rate, date: json.date, source: "frankfurter" };
  } catch {
    return { rate: FALLBACK_RATE, date: "fallback", source: "fallback" };
  }
}

const PALETTE = [
  "var(--accent-blue, #3b82f6)",
  "var(--accent-violet, #8b5cf6)",
  "var(--accent-emerald, #10b981)",
  "var(--accent-amber, #f59e0b)",
  "var(--accent-rose, #f43f5e)",
  "var(--accent-cyan, #06b6d4)",
];

function SparkChart({
  days,
  series,
}: {
  days: string[];
  series: ModelSeries[];
}) {
  const width = 720;
  const height = 220;
  const padX = 36;
  const padY = 20;
  const innerW = width - padX * 2;
  const innerH = height - padY * 2;
  const n = days.length;
  if (n === 0 || series.length === 0) {
    return (
      <svg
        role="img"
        aria-label="Spend chart"
        viewBox={`0 0 ${width} ${height}`}
        className="w-full"
      >
        <text
          x={width / 2}
          y={height / 2}
          textAnchor="middle"
          className="fill-[var(--text-muted)] text-xs"
        >
          No data in this window
        </text>
      </svg>
    );
  }
  const max = Math.max(
    ...series.flatMap((s) => s.points.map((p) => p.cost)),
    0.000001,
  );
  const stepX = n > 1 ? innerW / (n - 1) : 0;
  const xOf = (i: number) => padX + i * stepX;
  const yOf = (v: number) => padY + innerH - (v / max) * innerH;

  const ticks = 4;
  const tickValues = Array.from({ length: ticks + 1 }, (_, i) => (max * i) / ticks);

  return (
    <svg
      role="img"
      aria-label="Spend chart"
      viewBox={`0 0 ${width} ${height}`}
      className="w-full"
    >
      {tickValues.map((tv, i) => (
        <g key={`g-${i}`}>
          <line
            x1={padX}
            x2={width - padX}
            y1={yOf(tv)}
            y2={yOf(tv)}
            stroke="var(--border)"
            strokeDasharray="2 3"
            strokeWidth={0.5}
          />
          <text
            x={padX - 6}
            y={yOf(tv) + 4}
            textAnchor="end"
            className="fill-[var(--text-muted)] text-[10px]"
          >
            {gbp(tv)}
          </text>
        </g>
      ))}
      {days.map((d, i) => (
        <text
          key={`d-${i}`}
          x={xOf(i)}
          y={height - 4}
          textAnchor="middle"
          className="fill-[var(--text-muted)] text-[10px]"
        >
          {d.slice(5)}
        </text>
      ))}
      {series.map((s, idx) => {
        const d = s.points
          .map((p, i) => `${i === 0 ? "M" : "L"}${xOf(i)},${yOf(p.cost)}`)
          .join(" ");
        const colour = PALETTE[idx % PALETTE.length];
        return (
          <g key={`${s.model}-${s.reasoningEffort ?? ""}`}>
            <path
              d={d}
              fill="none"
              stroke={colour}
              strokeWidth={2}
              strokeLinejoin="round"
              strokeLinecap="round"
            />
            {s.points.map((p, i) =>
              p.cost > 0 ? (
                <circle
                  key={`p-${i}`}
                  cx={xOf(i)}
                  cy={yOf(p.cost)}
                  r={3}
                  fill={colour}
                />
              ) : null,
            )}
          </g>
        );
      })}
    </svg>
  );
}

export function SpendDashboard() {
  const [rows, setRows] = useState<SpendRow[]>([]);
  const [info, setInfo] = useState<ProxyInfo | null>(null);
  const [series, setSeries] = useState<SpendSeries | null>(null);
  const [windowKey, setWindowKey] = useState<WindowKey>("today");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const [fx, setFx] = useState<FxRate>({
    rate: FALLBACK_RATE,
    date: "fallback",
    source: "fallback",
  });

  const loadSeries = useCallback(async (w: WindowKey) => {
    try {
      return await invoke<SpendSeries>("api_spend_series", { window: w });
    } catch (e) {
      setError(String(e));
      return null;
    }
  }, []);

  const load = useCallback(
    async (w: WindowKey = windowKey) => {
      setBusy(true);
      try {
        const [next, proxy, s] = await Promise.all([
          invoke<SpendRow[]>("api_spend_summary"),
          invoke<ProxyInfo>("api_spend_proxy_info"),
          loadSeries(w),
        ]);
        setRows(next);
        setInfo(proxy);
        if (s) setSeries(s);
        setError("");
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [loadSeries, windowKey],
  );

  useEffect(() => {
    void loadFxRate().then(setFx);
  }, []);

  useEffect(() => {
    void load(windowKey);
    const timer = setInterval(() => void load(windowKey), 5000);
    return () => clearInterval(timer);
  }, [load, windowKey]);

  const action = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await fn();
      await load(windowKey);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const tokens = useMemo(
    () => rows.reduce((s, r) => s + r.inputTokens + r.outputTokens, 0),
    [rows],
  );
  const usdTotal = series?.totalCost ?? 0;
  const gbpTotal = usdTotal * fx.rate;

  return (
    <section className="min-h-0 flex-1 overflow-auto py-5 text-[var(--text-primary)]">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h1 className="flex items-center gap-2 text-xl font-semibold">
            <Coins size={22} /> Spend
          </h1>
          <p className="mt-1 text-xs text-[var(--text-muted)]">
            Reported via /v1/usage by configured providers · converted to GBP at 1 USD ={" "}
            {fx.rate.toFixed(4)} GBP
            {fx.source === "frankfurter"
              ? ` (frankfurter.dev ${fx.date})`
              : " (offline fallback)"}
          </p>
        </div>
        <button
          aria-label="Refresh spend"
          disabled={busy}
          onClick={() => void action(() => load(windowKey))}
          className="rounded-lg border border-[var(--border)] p-2"
        >
          <RefreshCw size={16} />
        </button>
      </div>

      <ProviderStatusStrip />

      <CodexSubscription />

      <div className="mb-4 flex flex-wrap gap-2">
        {WINDOWS.map((w) => (
          <button
            key={w.key}
            aria-pressed={windowKey === w.key}
            onClick={() => setWindowKey(w.key)}
            className={`rounded-lg border border-[var(--border)] px-3 py-2 ${
              windowKey === w.key ? "bg-[var(--bg-active)]" : ""
            }`}
          >
            {w.label}
          </button>
        ))}
      </div>

      <div className="mb-5 grid grid-cols-3 gap-3">
        <div className="rounded-xl border border-[var(--border)] p-4">
          <p className="text-xs text-[var(--text-muted)]">
            {WINDOWS.find((w) => w.key === windowKey)?.label} cost
          </p>
          <strong className="text-2xl">{gbp(gbpTotal)}</strong>
          <p className="text-xs text-[var(--text-muted)]">
            {usd(usdTotal)} USD
          </p>
        </div>
        <div className="rounded-xl border border-[var(--border)] p-4">
          <p className="text-xs text-[var(--text-muted)]">Tokens used</p>
          <strong className="text-2xl">{tokens.toLocaleString()}</strong>
        </div>
        <div className="rounded-xl border border-[var(--border)] p-4">
          <p className="text-xs text-[var(--text-muted)]">Sessions</p>
          <strong className="text-2xl">
            {rows.reduce((s, r) => s + r.sessionCount, 0).toLocaleString()}
          </strong>
        </div>
      </div>

      <div className="mb-5 rounded-xl border border-[var(--border)] p-4">
        <h2 className="text-sm font-medium">
          Cost by model
          {series && (
            <span className="ml-2 text-xs font-normal text-[var(--text-muted)]">
              {series.from} → {series.to}
            </span>
          )}
        </h2>
        <p className="mt-1 text-xs text-[var(--text-muted)]">
          Only models active in this window are shown.
        </p>
        {series && series.series.length > 0 ? (
          <>
            <div className="mt-3 flex flex-wrap gap-3 text-xs">
              {series.series.map((s, i) => (
                <span key={`legend-${i}`} className="inline-flex items-center gap-1">
                  <span
                    className="inline-block h-2 w-3 rounded-sm"
                    style={{ backgroundColor: PALETTE[i % PALETTE.length] }}
                  />
                  <span className="font-medium">{s.model}</span>
                  {s.reasoningEffort && (
                    <span className="text-[var(--text-muted)]">
                      · {s.reasoningEffort}
                    </span>
                  )}
                  <span className="text-[var(--text-muted)]">
                    {gbp(s.totalCost * fx.rate)} · {s.totalRequests} req
                  </span>
                </span>
              ))}
            </div>
            <div className="mt-3">
              <SparkChart days={series.days} series={series.series} />
            </div>
          </>
        ) : (
          <p className="mt-3 text-sm text-[var(--text-muted)]">
            No reported usage in this window yet. Have an integration POST a usage
            record to <code>http://127.0.0.1:4000/v1/usage</code> with the bearer
            token below to populate the chart.
          </p>
        )}
      </div>

      {error && (
        <p role="alert" className="mb-3 text-[var(--accent-red)]">
          {error}
        </p>
      )}

      <div className="mb-5 overflow-x-auto rounded-xl border border-[var(--border)]">
        <table className="w-full text-left text-sm">
          <thead className="bg-[var(--bg-secondary)] text-xs text-[var(--text-muted)]">
            <tr>
              {[
                "Provider",
                "Input",
                "Output",
                "Cost (USD)",
                "Requests",
                "Sessions",
                "Reasoning",
                "First seen",
                "Last seen",
              ].map((h) => (
                <th key={h} className="p-3">
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.length === 0 ? (
              <tr>
                <td
                  colSpan={9}
                  className="p-6 text-center text-[var(--text-muted)]"
                >
                  No data
                </td>
              </tr>
            ) : (
              rows.map((r) => (
                <tr key={r.provider} className="border-t border-[var(--border)]">
                  <td className="p-3">{r.provider}</td>
                  <td className="p-3">{r.inputTokens.toLocaleString()}</td>
                  <td className="p-3">{r.outputTokens.toLocaleString()}</td>
                  <td className="p-3">{usd(r.cost)}</td>
                  <td className="p-3">{r.requests.toLocaleString()}</td>
                  <td className="p-3">{r.sessionCount.toLocaleString()}</td>
                  <td className="p-3 text-[var(--text-muted)]">
                    {r.reasoningEfforts.length === 0
                      ? "—"
                      : r.reasoningEfforts.join(", ")}
                  </td>
                  <td className="p-3 text-[var(--text-muted)]">
                    {r.firstSeen ? new Date(r.firstSeen).toLocaleString() : "—"}
                  </td>
                  <td className="p-3 text-[var(--text-muted)]">
                    {r.lastSeen ? new Date(r.lastSeen).toLocaleString() : "—"}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      <div className="rounded-xl border border-[var(--border)] p-4 text-sm">
        <h2 className="font-medium">Local proxy</h2>
        <p className="mt-1 break-words text-xs text-[var(--text-muted)]">
          {info?.url ?? "Loading…"} · {info?.status ?? "starting"}
        </p>
        <p className="mt-2 text-xs text-[var(--text-muted)]">
          <code>/v1/usage</code> records reported usage. The per-run bearer token
          protects writes. No prompt text, API credentials or completion content
          is stored in the ledger.
        </p>
        <button
          disabled={!info}
          className="mt-3 inline-flex items-center gap-2 rounded-lg border border-[var(--border)] px-3 py-2"
          onClick={() =>
            void action(async () => {
              await navigator.clipboard.writeText(info!.token);
              setCopied(true);
            })
          }
        >
          <Copy size={14} />
          {copied ? "Token copied" : "Copy integration token"}
        </button>
      </div>

      <p className="mt-4 text-xs text-[var(--text-muted)]">
        Spend rolls up from usage reported to /v1/usage. Providers you add in
        Settings → AI & Integrations appear above with their status. Subscription
        limits (Claude/Codex) stay on the card above and do not roll up here.
      </p>
    </section>
  );
}