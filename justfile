default:
    @just --list

# rustfmt.toml (shared with moltis) uses unstable options, so formatting runs on
# nightly while builds, clippy and tests use the stable rust-toolchain.toml.
nightly_toolchain := "nightly-2026-06-20"

# Browse your archive ($MSGVAULT_HOME or ~/.msgvault); pass --demo, --sqlite, --url ...
run *args:
    cargo run --release -p msgvault-gpui -- {{args}}

# Open a generated demo archive.
demo:
    cargo run --release -p msgvault-gpui -- --demo

# Write a synthetic archive (real msgvault schema) into DIR.
fixture dir="demo-archive":
    cargo run -p msgvault-core --bin msgvault-fixture -- {{dir}}

# Render the real window headlessly (Metal) against the demo archive.
screenshot out="docs/screenshots/inbox.png" *args:
    cargo run --release -p msgvault-gpui --features screenshot -- --screenshot {{out}} {{args}}

# Regenerate the README screenshots.
screenshots:
    just screenshot docs/screenshots/inbox.png
    just screenshot docs/screenshots/inbox-dark.png --dark --select 3
    just screenshot docs/screenshots/newsletter.png --search from:hello@rustweekly.dev
    just screenshot docs/screenshots/chat.png --mailbox whatsapp --dark

# Format Rust (nightly rustfmt) and TOML (taplo).
format:
    cargo +{{nightly_toolchain}} fmt --all
    taplo fmt

# Check Rust and TOML formatting without writing.
format-check:
    cargo +{{nightly_toolchain}} fmt --all -- --check
    taplo fmt --check

# Verify Cargo.lock is in sync with workspace manifests.
lockfile-check:
    cargo fetch --locked

lint: lockfile-check
    cargo clippy --locked --workspace --all-targets --all-features -- -D warnings

test:
    cargo test --locked --workspace
    cargo test --locked --workspace --all-features

# License, advisory, ban and source checks over the dependency graph.
deny:
    cargo deny check

ci: format-check lint test

# Refresh the vendored msgvault schema used by fixtures and tests.
vendor-schema msgvault="../msgvault":
    cp {{msgvault}}/internal/store/schema.sql crates/msgvault-core/schema/schema.sql
    cp {{msgvault}}/internal/store/schema_sqlite.sql crates/msgvault-core/schema/schema_sqlite.sql
    @echo "Vendored schema from $(git -C {{msgvault}} rev-parse --short HEAD); update schema/README.md"
