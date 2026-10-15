import { spawn } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const child = spawn(process.platform === "win32" ? "npm.cmd" : "npm", ["run", "tauri", "--", "dev"], {
  cwd: resolve(root, "desktop/frontend"), shell: process.platform === "win32", stdio: "inherit", env: { ...process.env, LPM_VERSION: "0.2.0-astra" },
});
child.on("exit", code => process.exit(code ?? 1));
