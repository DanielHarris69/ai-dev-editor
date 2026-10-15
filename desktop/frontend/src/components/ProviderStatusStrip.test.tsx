// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const commands = vi.hoisted(() => ({
  ListProviders: vi.fn(),
  ProviderReadiness: vi.fn(),
}));
vi.mock("../../bridge/commands", () => commands);

import { ProviderStatusStrip } from "./ProviderStatusStrip";

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
  commands.ListProviders.mockReset().mockResolvedValue([]);
  commands.ProviderReadiness.mockReset().mockResolvedValue([]);
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

const flush = () => new Promise((r) => setTimeout(r, 0));

it("renders nothing when no provider is configured", async () => {
  await act(async () => {
    root.render(<ProviderStatusStrip />);
    await flush();
  });
  expect(host.textContent).toBe("");
});

it("shows each provider with its label and readiness text", async () => {
  commands.ListProviders.mockResolvedValue([{ id: "vertex-main", label: "Vertex (Konton)" }]);
  commands.ProviderReadiness.mockResolvedValue([
    { id: "vertex-main", readiness: "needsCredentials", credentialHint: "", env: [] },
  ]);
  await act(async () => {
    root.render(<ProviderStatusStrip />);
    await flush();
  });
  expect(host.textContent).toContain("Vertex (Konton)");
  expect(host.textContent).toContain("Add credentials in Settings");
});

it("describes a saved-credential provider as not yet connected", async () => {
  commands.ListProviders.mockResolvedValue([{ id: "devpass", label: "DevPass" }]);
  commands.ProviderReadiness.mockResolvedValue([
    { id: "devpass", readiness: "notConnected", credentialHint: "", env: [] },
  ]);
  await act(async () => {
    root.render(<ProviderStatusStrip />);
    await flush();
  });
  expect(host.textContent).toContain("live usage not yet connected");
});
