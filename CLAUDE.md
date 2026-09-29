# CLAUDE.md

- Windows-only. `build.bat` makes the release exe (Docker cross-compile, icon embedded) at `target/docker/Plasmer.exe`.
- No tests.
- `APP_VERSION` in `main.rs` is bumped by hand for each release; the updater compares it against the remote API.
