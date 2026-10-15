> Update 15 October: the spend dashboard is multi-provider. Providers are configured in Settings → AI & Integrations and stored in `providers.json`. Vertex and DevPass are configurable and report their readiness, but live usage pullers are not yet built. The demo seed data and mock proxy routes have been removed. Claude/Codex subscription tracking is retained. See [the spend ledger guide](#what-the-spend-ledger-measures) for the current shape.

# Astra Dev Editor MVP

An independent local derivative of [gug007/lpm](https://github.com/gug007/lpm), preserving LPM's project manager, terminal sessions, styling, file editor and Claude/Codex session analytics. GitHub fork: https://github.com/DanielHarris69/ai-dev-editor. Upstream MIT license and attribution retained.

## Open it

On this Mac, the built application is at:

`desktop/frontend/src-tauri/target/release/bundle/macos/Astra Dev Editor.app`

Double-click the app in Finder. Add this repository as a project, select **IDE / Files**, and open `examples/welcome.ts`. Select **Terminal** to return to your terminal panes. File editing uses Monaco and LPM's existing conditional-save/conflict protection; switching between modes keeps the editor buffer mounted. The selected mode is saved per project. It is a Monaco-based workspace, not the VS Code extension host.

Choose **Spend dashboard** above the workspace. The dashboard shows usage reported to the local `/v1/usage` proxy, with a status strip for each configured provider. Providers are set up in **Settings → AI & Integrations → Spend providers**. The four time windows are today, this week, this month and last month; the chart only includes models that had at least one call in the selected window. Totals convert USD to GBP at the live [frankfurter.dev](https://api.frankfurter.dev) rate (cached for the session). Missing usage shows `No data` rather than a fabricated zero. The Claude/Codex subscription card sits above the chart — it stays for the day Work provisions accounts, but its numbers do not roll up into the spend totals.

To develop:

```sh
node scripts/astra-dev.mjs
```

Or from `desktop/frontend`:

```sh
npm ci
LPM_VERSION=0.1.0-astra npm run tauri -- dev
LPM_VERSION=0.1.0-astra npm run tauri -- build --bundles app --no-sign --ci
```

PowerShell users can set `$env:LPM_VERSION = '0.1.0-astra'` before running npm commands, or use the cross-platform Node launcher.

## Architecture and platform choice

Reusing the existing Tauri v2 / React / Vite application preserves the requested LPM experience. A new scaffold would discard working project, PTY, theme and file-editing infrastructure. Tailwind, lucide-react, Monaco and rusqlite were already dependencies. Axum uses Hyper to provide the new HTTP routes. Shared code is retained for macOS, Windows and Linux; the added features do not require platform-specific implementations. The marginal implementation effort is estimated below the requested 10% threshold. This is an engineering estimate, not a measured delivery cost. macOS is the native build target on this machine; a Windows cross-check does not establish Windows or Linux runtime acceptance.

- `src/components/WorkspaceMode.tsx`: explicit Terminal/IDE choice.
- `src/components/files/FilesPane.tsx`: reused project file tree and Monaco editor.
- `src/components/SpendDashboard.tsx`: aggregated token/cost ledger and proxy status.
- `src-tauri/src/spend.rs`: SQLite database, validation, deduplication and Tauri commands.
- `src-tauri/src/spend_proxy.rs`: loopback HTTP usage ingestion (`/health`, `/v1/usage`).
- `src-tauri/src/providers.rs`: provider registry in `providers.json`; credentials are stored in the Keychain, never in the file.
- `src-tauri/src/provider_connectors.rs`: per-kind readiness rules and the environment and credential contract for each kind.

The derivative's default data directory is `~/.astra-dev-editor`, overridden by `LPM_DIR`. It contains its own projects, settings, socket and `api-spend.db`. Child processes inherit this data-directory choice. Existing project repo configuration files remain `.lpm.yml` for compatibility. Automatic installation/repair of LPM CLI symlinks, agent hooks and shared skills is disabled for the derivative. Automatic upstream updates are disabled, and custom versions are not offered an upstream replacement. Existing manual settings actions are inherited; do not use their CLI/hook install buttons until their namespacing is adapted.

## What the spend ledger measures

`api_spend` records `request_id`, `provider`, `model`, `input_tokens`, `output_tokens`, `cost` (USD), UTC `timestamp`, optional `session_id`, `reasoning_effort` (one of `low|medium|high|minimal|none`) and `agent`. The MVP records only requests reported to it and cannot observe Gemini app conversations, Vertex billing, Hermes usage or other tools merely because those tools are running. Costs are caller-reported USD values; the dashboard converts to GBP at the live [frankfurter.dev](https://api.frankfurter.dev) rate (cached for the session, offline fallback to 0.79). The dashboard's v1 windows are today, this week, this month and last month; the chart shows only models active in the selected window. Subscription usage limits remain a separate measurement from token-based API costs.

## HTTP integration

The application starts a server on `127.0.0.1:4000`. If the port is occupied, the app remains usable and the dashboard displays `unavailable` with the bind error. It does not pick an unannounced port. `/health` reports service identity. Writes require a per-run bearer token, available through **Copy integration token** in the dashboard. The token is held in memory and changes when the process restarts. No prompt text, API credentials or completion content is stored in the ledger. Requests are limited to 64 KiB. There is no CORS grant or external-network bind.

Set `ASTRA_PROXY_TOKEN` in your integration process using the copied token, then POST real usage records:

```sh
curl --fail-with-body http://127.0.0.1:4000/v1/usage \
  -H "Authorization: Bearer $ASTRA_PROXY_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "request_id":"globally-unique-provider-response-id",
    "provider":"hermes",
    "model":"google/gemini-3.8-flash",
    "input_tokens":123,
    "output_tokens":45,
    "cost":0.002,
    "session_id":"hermes-session-123",
    "reasoning_effort":"low",
    "agent":"hermes"
  }'
```

`provider` must be `hermes`, or the kind of an enabled provider configured in Settings (`vertex` or `devpass`; use the configured identifier for an Other provider). `reasoning_effort` is one of `low|medium|high|minimal|none`. The numeric example above illustrates the request shape; it is not a published model price. Identical retries do not add spend twice. Reusing an ID with different usage fails. Tokens must be non-negative integers and costs finite, non-negative numbers; unknown providers and empty IDs/models fail. The endpoint trusts the integration's cost attribution: `reported` does not imply independently verified billing. Stream chunks need aggregation by the integration before submitting a single final usage record. Retry failed deliveries from the integration; this MVP does not include a queue. The dashboard chart only shows a model if it had at least one call in the active window.

## Provider configuration

Add a provider in **Settings → AI & Integrations → Spend providers**. Each has an identifier, label, kind, optional model allowlist, an enabled flag and an optional credential.

| Kind | Source | Credential | Environment hints | Status |
| --- | --- | --- | --- | --- |
| `reported` | Pushed with the configured identifier | None | — | Generic integration reports usage |
| `hermes` | Pushed to `/v1/usage` | None | — | Reports usage |
| `vertex` | Pulled from Google Cloud | Service-account JSON path, or blank for gcloud application-default credentials | `ASTRA_VERTEX_PROJECT`, `ASTRA_VERTEX_SA_PATH` | Credentials can be saved; live pull not built |
| `devpass` | Pulled from DevPass | DevPass API token | `ASTRA_DEVPASS_ENDPOINT`, `ASTRA_DEVPASS_TOKEN` | Credentials can be saved; live pull not built |

Credentials go to the macOS login Keychain under `provider:<id>`. The dashboard never reads a saved credential back. `providers.json` holds only non-secret settings and is written with owner-only permissions. A disabled provider no longer accepts new usage records when no other enabled configuration of that kind permits them. Model allowlists are enforced at ingestion; existing ledger history is preserved. Hermes remains accepted without setup unless explicitly configured and disabled. The dashboard still lists it in the status strip, marked as disabled.

Until a live puller exists, a pulled provider shows `Credentials saved; live usage not yet connected`. No usage is fabricated for it.

## Next implementation boundaries

1. Live Vertex and DevPass usage pullers, once credentials are provided.
2. Named custom time windows in the dashboard (currently today / this week / this month / last month).
3. Namespaced CLI/hook installation and independent updater/release channels.
4. Windows and Linux native installer/UI/terminal testing, macOS signing/notarization, and inherited dependency advisory remediation before production distribution.

See `VERIFICATION.md` for evidence and the remaining acceptance gaps.
