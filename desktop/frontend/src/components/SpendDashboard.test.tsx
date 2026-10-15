// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { SpendDashboard } from "./SpendDashboard";

vi.mock("./CodexSubscription", () => ({ CodexSubscription: () => null }));

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

let host: HTMLDivElement;
let root: Root;

const baseSeries = (window: string): unknown => ({
  window,
  from: "2026-10-13",
  to: "2026-10-13",
  totalCost: 0.01,
  totalRequests: 1,
  days: ["2026-10-13"],
  series: [
    {
      model: "google/gemini-3.8-flash",
      reasoningEffort: "low",
      totalCost: 0.01,
      totalRequests: 1,
      points: [{ day: "2026-10-13", cost: 0.01, requests: 1 }],
    },
  ],
  tokensByDay: [150],
  costByDay: [0.01],
});

beforeEach(() => {
  (
    globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  invoke.mockImplementation(async (name: string, args?: { window?: string }) => {
    if (name === "api_spend_summary") {
      return [
        {
          provider: "hermes",
          inputTokens: 100,
          outputTokens: 50,
          cost: 0.01,
          requests: 1,
          sessionCount: 1,
          firstSeen: "2026-10-13T10:00:00Z",
          lastSeen: "2026-10-13T10:00:00Z",
          reasoningEfforts: ["low"],
        },
      ];
    }
    if (name === "api_spend_proxy_info") {
      return { url: "http://127.0.0.1:4000", status: "listening", token: "test-token" };
    }
    if (name === "api_spend_series") {
      return baseSeries(args?.window ?? "today");
    }
    return null;
  });
  sessionStorage.clear();
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
  vi.clearAllMocks();
});

it("renders the Spend title and gbp total for the default today window", async () => {
  await act(async () => {
    root.render(<SpendDashboard />);
  });
  expect(host.textContent).toContain("Spend");
  expect(host.textContent).not.toContain("Hermes spend");
  // USD 0.01 * 0.79 fallback = £0.0079
  expect(host.textContent).toContain("£0.0079");
  expect(host.textContent).toContain("google/gemini-3.8-flash");
  expect(host.textContent).toContain("low");
  expect(host.textContent).not.toContain("test-token");
});

it("switching to last month issues a new api_spend_series request with that window", async () => {
  await act(async () => {
    root.render(<SpendDashboard />);
  });
  const lastMonthBtn = [...host.querySelectorAll("button")].find(
    (b) => b.textContent === "Last month",
  )!;
  await act(async () => {
    lastMonthBtn.click();
  });
  const calls = invoke.mock.calls.filter(([n]) => n === "api_spend_series");
  expect(calls.at(-1)?.[1]).toEqual({ window: "last_month" });
});

it("shows an error and no data when the backend fails without inventing a total", async () => {
  invoke.mockRejectedValue(new Error("database unavailable"));
  await act(async () => {
    root.render(<SpendDashboard />);
  });
  expect(host.querySelector('[role="alert"]')?.textContent).toContain(
    "database unavailable",
  );
  expect(host.textContent).toContain("No data");
  // The empty-state total must be zero pounds, not a fabricated non-zero value.
  expect(host.textContent).toMatch(/£0\.0000/);
  expect(host.textContent).not.toMatch(/£0\.[1-9]/);
});

it("renders an empty-state message when the series has no active models", async () => {
  invoke.mockImplementation(async (name: string, args?: { window?: string }) => {
    if (name === "api_spend_proxy_info")
      return { url: "http://127.0.0.1:4000", status: "listening", token: "x" };
    if (name === "api_spend_series") {
      return {
        window: args?.window ?? "today",
        from: "2026-10-13",
        to: "2026-10-13",
        totalCost: 0,
        totalRequests: 0,
        days: ["2026-10-13"],
        series: [],
        tokensByDay: [0],
        costByDay: [0],
      };
    }
    return [];
  });
  await act(async () => {
    root.render(<SpendDashboard />);
  });
  expect(host.textContent).toContain("Only models active in this window are shown");
  expect(host.textContent).toContain("No reported usage in this window yet");
});