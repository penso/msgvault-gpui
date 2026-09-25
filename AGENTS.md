# AGENTS.md

Guidance for agents working in this repository.

## Architecture

msgvault-gpui is a read-only native viewer for msgvault archives. msgvault
(Go, `../msgvault`) owns the data. Never modify it from here, and never write
to a user's archive.

| Location | Responsibility |
| --- | --- |
| `crates/msgvault-core` | Archive location/config, domain model, the `Backend` trait with `SqliteBackend` (direct read-only DB) and `HttpBackend` (msgvault daemon REST API), fixtures |
| `crates/msgvault-gpui` | GPUI app built on gpui-kit: `app/` holds state and loading (`mod.rs`) and views (`sidebar`, `list`, `reader`, `titlebar`, `welcome`) |

- Dependencies point from the UI to core. `msgvault-core` must not depend on
  GPUI.
- Both backends must return the same model. When you add a capability, add
  it to both, or make it degrade explicitly (for example, `MailboxCounts::kinds`
  is empty over HTTP).
- **SQLite reads:**
  - Filter out `m.deleted_at IS NOT NULL` (dedup losers).
  - Treat NULL or empty `message_type` as email.
  - Probe optional columns and tables (`sqlite::Schema`) instead of assuming
    the newest schema.
  - Resolve label ids first, then filter with `ml.label_id IN (...)`.
- **Attachments** are content-addressed. Always verify the SHA-256, keep the
  size limits, and treat filenames as untrusted (`format::safe_filename`).
- **HTTP.** Follow `../msgvault/api/openapi.yaml` and tolerate unknown or
  missing fields (`#[serde(default)]`).

## Rust conventions

- Each crate exposes its own typed `Error`/`Result` (`thiserror`). `anyhow`
  is used only at the app boundary.
- No `unwrap`/`expect` in production code; the workspace lints deny them.
  No `unsafe`.
- Keep dependency versions in the root `[workspace.dependencies]`.
- Prefer enums for closed sets (`Mailbox`, `BackendChoice`). Parse strings at
  the boundaries.

## GPUI rules

- Never block the UI thread. Every backend call goes through `MailApp::run`
  (a background executor task, then `this.update`). Generation counters drop
  stale results.
- Render from prepared state only. Don't query in `render`.
- **Imports.** Import gpui through `gpui_kit::*`, and component items
  explicitly (`gpui_kit::component::...`). A glob-import of both makes names
  like `Size` and `red()` ambiguous.
- **Icons.** Icons beyond gpui-component's default 101 must be listed in
  `assets.rs` (`icon_assets!`), or they render blank.
- **Theme.** Change the theme through `crate::theme::apply`, so the blue
  accent is re-applied after `Theme::change`.

## Tooling

The lint and format configuration is shared with moltis and herdr-gpui:

| File | Purpose |
| --- | --- |
| `rustfmt.toml` | Uses moltis's unstable options, so `just format` runs rustfmt from the pinned nightly (`nightly_toolchain` in the justfile). Plain `cargo fmt` on stable ignores them and warns. |
| `clippy.toml` | Moltis thresholds. Don't add `#[allow(clippy::too_many_arguments)]` or `type_complexity`. |
| `[workspace.lints]` | Denies `unsafe_code`, `unused_qualifications`, `unwrap_used` and `expect_used`. |
| `taplo.toml` | TOML formatting: aligned entries, sorted keys and arrays. Run it through `just format`. |
| `deny.toml` | Licenses, advisories, bans and sources (`just deny`). A new licence gets a scoped `[[licenses.exceptions]]` entry with a reason. |
| `rust-toolchain.toml` / `mise.toml` | The stable compiler for builds, clippy and tests. Keep the two in sync. |

## Verification

- Run `just format`, then `just ci`: nightly rustfmt and taplo checks, a
  lockfile check, clippy with `-D warnings` and all features, and tests on
  default and all features.
- When dependencies change, also run `just deny`.
- For UI changes, render with `just screenshot /tmp/x.png [--dark] [--search Q]
  [--mailbox NAME] [--select N]` and look at the image.
- Core behavior is tested against fixtures built from the vendored real
  schema (`tests/sqlite.rs`) and a mock daemon (`tests/http.rs`).
