# Contributing

Thanks for your interest in improving **gibbon**! This is an unofficial,
community project and is not affiliated with example.com.

## Ground rules

- Be respectful. Do not use this project to redistribute the station's audio or
  to imply any official relationship with example.com.
- By contributing you agree your work is licensed under the MIT license
  (see [README](README.md#license)).

## Development setup

```bash
# Linux build deps
sudo apt-get install libgtk-3-dev libayatana-appindicator3-dev libasound2-dev pkg-config

cargo build          # compile
cargo test           # offline unit tests
cargo run            # run the tray app
cargo run -- --selftest   # headless audio-pipeline check
```

## Before you open a PR

Run the same checks CI runs — they must all pass:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features   # treated as -D warnings in CI
cargo test --all-features
```

## Commit / PR guidelines

- Keep changes focused; describe the *why* in the PR body.
- Match the surrounding code style (the codebase is `rustfmt`-formatted).
- Add or update tests for logic changes where practical.
- For user-facing changes, add an entry to [CHANGELOG.md](CHANGELOG.md) under
  *Unreleased*.

## Cutting a release (maintainers)

```bash
# bump `version` in Cargo.toml, update CHANGELOG.md, then:
git tag vX.Y.Z
git push origin vX.Y.Z
```

The release workflow builds and publishes packages for all platforms.
