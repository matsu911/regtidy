# regtidy

CLI tool to clean up unused Docker images from private Registry V2 instances.

Deletes old, excess, or pattern-matched tags while preserving images referenced by kept tags via shared-digest safety checks.

## Installation

```bash
cargo install --path .
```

## Usage

Set the registry URL via `--registry` flag or `REGTIDY_REGISTRY` environment variable.

### List repositories and tags

```bash
regtidy --registry http://localhost:5000 list
regtidy --registry http://localhost:5000 --repo myapp list
```

### Find dangling repositories (no tags)

```bash
regtidy --registry http://localhost:5000 dangling
```

### Clean up tags

Exactly one strategy is required: `--keep`, `--older-than`, or `--pattern`.

```bash
# Keep the 5 most recent tags, delete the rest
regtidy --registry http://localhost:5000 --repo myapp clean --keep 5

# Delete tags older than 30 days
regtidy --registry http://localhost:5000 --repo myapp clean --older-than 30

# Delete tags matching a regex pattern
regtidy --registry http://localhost:5000 --repo myapp clean --pattern "^dev-"

# Preview changes without deleting
regtidy --registry http://localhost:5000 --repo myapp clean --keep 5 --dry-run
```

Omit `--repo` to process all repositories in the registry.

## Safety

- **Shared-digest protection**: If a tag marked for deletion shares a digest with a kept tag, it is automatically preserved.
- **Conservative defaults**: Tags with unknown creation dates are kept, not deleted.
- **Dry run**: Use `--dry-run` to preview the full plan before making any changes.
- **Digest-level deletion**: Multiple tags pointing to the same digest result in a single DELETE request.

## Building from source

Requires Rust toolchain. A Nix Flakes dev environment is provided via `flake.nix` + `.envrc`.

```bash
cargo build --release
cargo test
```

## License

MIT
