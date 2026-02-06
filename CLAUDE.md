# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**clearnear** is a CLI tool written in Rust that cleans up unused Docker images from private Docker Registry V2 instances. It deletes old, excess, or pattern-matched tags while preserving images referenced by kept tags via shared-digest safety checks.

## Build & Run Commands

```bash
cargo build                  # Debug build
cargo build --release        # Release build
cargo test                   # Run tests (none currently exist)
cargo clippy                 # Lint
cargo fmt                    # Format
cargo run -- --registry http://localhost:5000 --repo myapp --keep 5 --dry-run  # Example run
```

The dev environment is managed via Nix Flakes (`flake.nix` + `.envrc` with direnv). It provides: Rust toolchain, rust-analyzer, clippy, rustfmt, pkg-config, openssl.

## Architecture

Entry point is `src/main.rs` with a `#[tokio::main]` async runtime. The flow is:

1. **cli.rs** — `clap::Parser` struct. Registry URL via `--registry` or `CLEARNEAR_REGISTRY` env var. Exactly one strategy must be chosen (mutually exclusive clap group).
2. **strategy.rs** — Three cleanup strategies: `KeepRecent(n)`, `OlderThan(days)`, `Pattern(regex)`. After partitioning tags, applies **shared-digest safety**: if a digest is referenced by any kept tag, all tags sharing that digest are moved to the keep list.
3. **registry.rs** — `RegistryClient` wrapping `reqwest`. Implements Docker Registry V2 API: catalog listing, tag listing, manifest/digest resolution, blob fetching (for image creation timestamps), and manifest deletion. All paginated via `Link` header parsing. `resolve_all_tags()` uses a tokio semaphore (limit 10) for bounded concurrency.
4. **models.rs** — Data types: `TagInfo` (tag + digest + created timestamp), `CleanupPlan` (to_delete + to_keep lists), and API response structs.
5. **output.rs** — Color-coded terminal output for cleanup plans and summary statistics.
6. **error.rs** — `AppError` enum via `thiserror` for validation errors. Runtime errors use `anyhow::Result`.

## Key Design Decisions

- Deletions operate on unique digests, not individual tags — multiple tags pointing to the same digest result in a single DELETE request.
- Conservative defaults: tags with unknown creation dates are kept (not deleted).
- `--dry-run` previews the full plan without making any API mutations.
- Exit code 1 if any errors occurred during processing.
