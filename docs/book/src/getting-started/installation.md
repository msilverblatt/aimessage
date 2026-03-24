# Installation

## Prerequisites

- macOS Ventura or later
- Rust toolchain: install via [rustup](https://rustup.rs/)
- Messages.app signed into an Apple ID

## Clone and build

```bash
git clone <repo-url>
cd aimessage
cargo build --release
```

The release binary is output to `target/release/aimessage`.

## Create the app bundle

Running AiMessage as a proper `.app` bundle is required for macOS to associate the Full Disk Access and Automation permissions with it. Running the bare binary from a terminal works only if you grant those permissions to your terminal emulator instead.

```bash
bash scripts/build-app.sh
```

This script compiles a release build and packages it into `bundle/AiMessage.app`. After it completes, grant the bundle Full Disk Access (see [Permissions](./permissions.md)) and then launch it:

```bash
open bundle/AiMessage.app
```

## What `build-app.sh` does

The script performs these steps:

1. Runs `cargo build --release`
2. Creates the `bundle/AiMessage.app/Contents/MacOS/` directory structure
3. Copies the compiled binary into the bundle
4. Writes a minimal `Info.plist`

You need to re-run the script any time you rebuild the binary with new changes.
