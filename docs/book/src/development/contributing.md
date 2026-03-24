# Contributing

## Getting started

1. Fork the repository and clone your fork.
2. Create a branch for your change.
3. Make your changes, run `cargo clippy` and `cargo test`.
4. Open a pull request against `main`.

## Code style

Follow the patterns already established in the codebase:

- Keep layers separate. The API layer should not import from `imessage/`. Everything goes through the `MessageBackend` trait.
- Use the existing error type. Add new variants to `core_layer/errors.rs` rather than introducing new error types.
- No `unwrap()` in production paths. Use `?` or explicit error handling.
- Run `cargo clippy` before pushing. CI will fail on clippy warnings.
- Format with `cargo fmt` before pushing.

## Where to find issues

Check the GitHub issues list for open bugs and feature requests. Issues tagged `good first issue` are a good starting point.

## Testing

Unit tests live alongside the code they test. Integration tests that require a real Mac (Full Disk Access, Messages.app) should be marked with `#[ignore]` so they do not run in CI automatically.

```bash
cargo test                    # run all non-ignored tests
cargo test -- --ignored       # run ignored (integration) tests on a Mac with permissions
```
