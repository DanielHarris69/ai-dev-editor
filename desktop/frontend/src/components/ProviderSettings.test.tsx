// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ProviderSettings } from "./ProviderSettings";

const commands = vi.hoisted(() => ({
  ListProviders: vi.fn(),
  SaveProvider: vi.fn(),
  RemoveProvider: vi.fn(),
}));
vi.mock("../../bridge/commands", () => commands);
vi.mock("sonner", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
  commands.ListProviders.mockReset().mockResolvedValue([]);
  commands.SaveProvider.mockReset().mockResolvedValue({});
  commands.RemoveProvider.mockReset().mockResolvedValue(undefined);
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

const flush = () => new Promise((r) => setTimeout(r, 0));

it("shows the empty state when no providers are configured", async () => {
  await act(async () => {
    root.render(<ProviderSettings />);
    await flush();
  });
  expect(host.textContent).toContain("No providers configured yet.");
  expect(commands.ListProviders).toHaveBeenCalledTimes(1);
});

it("lists providers without exposing credentials", async () => {
  commands.ListProviders.mockResolvedValue([
    {
      id: "vertex-main",
      label: "Vertex (Konton)",
      kind: "vertex",
      enabled: true,
      modelAllowlist: [],
      hasCredentials: true,
    },
  ]);
  await act(async () => {
    root.render(<ProviderSettings />);
    await flush();
  });
  expect(host.textContent).toContain("Vertex (Konton)");
  expect(host.textContent).toContain("credentials saved");
  expect(host.querySelector('input[type="password"]')).toBeNull();
});

it("sends the credential only on save and never as a list field", async () => {
  await act(async () => {
    root.render(<ProviderSettings />);
    await flush();
  });
  const addButton = Array.from(host.querySelectorAll("button")).find(
    (b) => b.textContent === "Add provider",
  )!;
  await act(async () => addButton.click());

  const inputs = host.querySelectorAll("input");
  const idInput = inputs[0] as HTMLInputElement;
  const labelInput = inputs[1] as HTMLInputElement;
  const pw = host.querySelector('input[type="password"]') as HTMLInputElement;
  const setValue = (el: HTMLInputElement, v: string) => {
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
    setter.call(el, v);
    el.dispatchEvent(new Event("input", { bubbles: true }));
  };
  await act(async () => {
    setValue(idInput, "vertex-main");
    setValue(labelInput, "Vertex (Konton)");
    setValue(pw, "/secret/sa.json");
  });
  const save = Array.from(host.querySelectorAll("button")).find((b) => b.textContent === "Save")!;
  await act(async () => {
    save.click();
    await flush();
  });

  expect(commands.SaveProvider).toHaveBeenCalledTimes(1);
  const [record, credential] = commands.SaveProvider.mock.calls[0];
  expect(record).toMatchObject({ id: "vertex-main", label: "Vertex (Konton)", kind: "vertex" });
  expect(record).not.toHaveProperty("credential");
  expect(credential).toBe("/secret/sa.json");
});
