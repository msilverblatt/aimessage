# Building from Source

## Debug build

```bash
cargo build
```

Output: `target/debug/aimessage`

## Release build

```bash
cargo build --release
```

Output: `target/release/aimessage`

## Run directly

```bash
cargo run
```

This requires Full Disk Access to be granted to your terminal emulator (not just the app bundle). Suitable for development; use the app bundle for regular use.

## Debug logging

AiMessage uses the `tracing` crate. Set `RUST_LOG` to control log output:

```bash
# Structured JSON logs for the aimessage crate only
RUST_LOG=aimessage=debug cargo run

# All crates at debug level
RUST_LOG=debug cargo run

# Trace-level (very verbose)
RUST_LOG=aimessage=trace cargo run
```

## Linting

```bash
cargo clippy
```

Fix all warnings before submitting a pull request. The CI enforces a clean clippy run.

## Build the app bundle

```bash
bash scripts/build-app.sh
```

This compiles a release build and packages it into `bundle/AiMessage.app`. Run this whenever you want to test with the proper macOS permissions setup, or to produce a distributable bundle.

## Running tests

```bash
cargo test
```

There are 16 unit tests covering core logic. No special setup is required — the tests do not need a running Messages.app or database access.
