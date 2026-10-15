import { Code2, Terminal } from "lucide-react";
interface Props {
  mode: string;
  onChange: (mode: "terminal" | "ide") => void;
}
export function WorkspaceMode({ mode, onChange }: Props) {
  return (
    <div
      aria-label="Workspace mode"
      className="my-2 flex shrink-0 gap-1 text-xs"
    >
      <button
        aria-pressed={mode === "terminal"}
        onClick={() => onChange("terminal")}
        className={`flex items-center gap-2 rounded-lg border border-[var(--border)] px-3 py-2 ${mode === "terminal" ? "bg-[var(--bg-active)]" : ""}`}
      >
        <Terminal size={14} />
        Terminal
      </button>
      <button
        aria-pressed={mode === "ide"}
        onClick={() => onChange("ide")}
        className={`flex items-center gap-2 rounded-lg border border-[var(--border)] px-3 py-2 ${mode === "ide" ? "bg-[var(--bg-active)]" : ""}`}
      >
        <Code2 size={14} />
        IDE / Files
      </button>
    </div>
  );
}
