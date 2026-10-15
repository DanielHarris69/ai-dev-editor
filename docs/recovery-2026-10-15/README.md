# Recovery checkpoint — 15 October 2026

This is the current handoff for Astra Dev Editor. Repository: `projects/ai_dev_editor`; branch: `codex/ai-dev-editor-mvp`. The separate `lpm_customisation` checkout is not the source for this build.

## Recovered agreement

Recovered read-only from Hermes session `20261015_130826_b95a36` (debugging-agent profile). Daniel approved continuing this branch, committing and pushing to `DanielHarris69/ai-dev-editor` after checks, a macOS build, Keychain storage, app-wide provider settings, and Vertex/DevPass connector stubs. Demo removal is scoped to Spend. Keep Terminal/Monaco IDE switching and the Claude/Codex subscription controls.

## Complete in source and local candidate

- Provider configuration in Settings → AI & Integrations → Spend providers. Add/edit/remove Hermes, Vertex, DevPass, or Other provider (reported usage); persist non-secret settings in `~/.astra-dev-editor/providers.json` and optional credentials through the platform credential store.
- Spend dashboard shows configured-provider readiness and reported usage totals, model chart and date windows. It never invents provider usage. Enable flags and model allowlists are enforced at ingestion. Other providers report using their configured identifier; Vertex/DevPass use their kind. Multiple configurations of the same kind share one ledger identity; any enabled configuration permitting the model accepts that record. Historical totals are preserved when disabled.
- Claude/Codex footer placeholders stay visible when enabled and open the existing subscription usage view. Existing local subscription reports still work. Those integrations use their existing authentication flows, not the spend-provider credential field.
- Spend demo commands, seed controls and screenshot removed. A transactional legacy-schema migration backs up the ledger, carries reported rows forward and removes demo rows from the active database on first launch. The existing user ledger was inspected read-only; its seven demo rows will be migrated when the candidate is opened. Browser fixtures use isolated synthetic data and are verification assets only.
- Native macOS Apple Silicon application built, version `0.3.0-astra`. Existing LPM CLI link preserved. No changes to the other LPM checkout.

## Verified in this recovery

| Check | Result | Evidence |
| --- | --- | --- |
| Frontend | 303 files, 3,358 tests passed | `evidence/frontend-tests.log` |
| Rust library | 1,538 passed, 5 ignored (Keychain test also run separately) | `evidence/rust-tests.log` |
| Native Keychain seam | Temporary unique credential saved, read, checked and deleted | `evidence/keychain-check.log` |
| Windows GNU cross-check | Passed; inherited `send_cmd` warning | `evidence/windows-check.log` |
| Production TypeScript/Vite + macOS bundle | Passed, unsigned | `evidence/macos-build.log` |
| Native startup | Legacy demo DB upgraded with backup to an empty active ledger; unauthenticated write rejected; busy-port app survives; LPM CLI unchanged | `evidence/native-smoke.json`; runner in `../astra-mvp/evidence/native-smoke.py` |
| Browser fixture | Spend/provider status rendering, no demo control, Monaco edits survive mode switches, no page errors | `evidence/browser-check.json`, `evidence/browser-check.mjs` |

The browser fixture mocks IPC and exchange-rate responses. It is not native UI acceptance or proof of a provider connection. The Keychain check runs the real Rust storage seam, not the packaged app's credential form. This candidate has not undergone Daniel's usability test or Windows/Linux native acceptance.

## Deliberately unfinished

Vertex and DevPass automatic collection is **not implemented**, as agreed in the session. Saving credentials does not connect them. The working integration path is authenticated, caller-reported `POST /v1/usage`; reported cost is not independently reconciled billing. Live connector work needs an agreed authoritative usage source and its authentication contract. Subscription plans and API billing remain separate.

The IDE uses Monaco, the editor component used by VS Code; it does not embed the VS Code extension host or vscode.dev. Distribution signing, inherited dependency advisories, and CLI/hook namespacing remain outside this local candidate.

## Resume here

Open `desktop/frontend/src-tauri/target/release/bundle/macos/Astra Dev Editor.app`, then Settings → AI & Integrations → Spend providers. Add Vertex or DevPass without a real credential to review the settings and honest disconnected status. Use a real integration later to populate Spend; no seed data is installed.

Unrelated untracked `.agents/` publishing/music tooling is excluded from the development commit. All prior MVP source needed by this branch is included so the checkout builds independently.
