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

Go to **Actions → Release → Run workflow** and enter the tag to create
(e.g. `v0.2.0`). No manual tagging or version bumping is needed — the workflow:

1. Bumps `version` in `Cargo.toml`/`Cargo.lock`, promotes the CHANGELOG's
   *Unreleased* section to the new version, and commits that to the branch.
2. Builds and packages for all platforms from that commit.
3. Creates the tag and a GitHub Release with auto-generated notes listing every
   change since the previous release.

Just collect user-facing changes under *Unreleased* in
[CHANGELOG.md](CHANGELOG.md) between releases as usual.
