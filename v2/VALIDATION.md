# Validation

Run on 2026-10-24 on macOS ARM64. Checkout baseline:
`c9a25126425af9a60e4835109761a267fcea6d2a`; beta API reference:
`fd300e276356587726566026f072563eb9c34cb1`.

| Check | Result |
| --- | --- |
| Rust 1.98.0 `cargo test --locked` | 13 tests pass, including real SDK/common tests |
| Rust 1.85.0 `cargo test --locked` | 13 tests pass |
| Both toolchains fmt and strict all-target Clippy | pass |
| Windows GNU all-target check and strict Clippy | pass; cross-compile only |
| Original JS recorded random tape oracle | 15 cases pass |
| Actual beta List/Form parser and reconciliation | pass; no DOM/desktop rendering |
| Full manifest Draft 2020-12 JSON Schema | pass |
| Byte-exact beta/original fixtures and icon SHA-256 | pass |
| Cargo package and extracted-package Rust tests | pass; fixtures/licenses included |
| Disposable V1 `pnpm test` / `pnpm typecheck` | pass / pass; source untouched |
| Host registration and full Tezbar rebuild | not run |
| Real macOS UI / OS clipboard QA | not run; clipboard IPC stubbed |
| Real Windows UI / OS clipboard QA | not run; cross-compile is not OS QA |

Run from this directory:

```sh
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo +1.85.0 test --locked
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --all-targets -- -D warnings
cargo check --locked --all-targets --target x86_64-pc-windows-gnu
cargo clippy --locked --all-targets --target x86_64-pc-windows-gnu -- -D warnings
node scripts/oracle.mjs --check
python3 -m pip install --no-cache-dir --target target/validation-python jsonschema==4.25.1
PYTHONPATH=target/validation-python python3 scripts/validate.py
cargo package --allow-dirty --locked
```

The archive under `target/package/` includes all test fixture licenses, pinned
sources, adapter, icon, manifest, documentation and Cargo.lock. Extract it into a
new directory under `target/` and run both toolchains' tests from there. Cargo
excludes the auxiliary nested `tests/tauri-stub` crate; the packaged harness uses
the small desktop emitter stub in `tests/beta_host/mod.rs` and needs no path
dependency or host checkout. Production integration uses real Tauri and SDK.

For renderer validation, make a disposable V1 source copy under `target/v1-check`
with root `package.json`, `package-lock.json`, `tsconfig.json`, `raycast-env.d.ts`,
`src/` and `test/`. Run `pnpm --dir target/v1-check install --ignore-scripts`,
then `pnpm --dir target/v1-check test` and `pnpm --dir target/v1-check typecheck`.
pnpm's peer-dependency warning did not fail either check; its disposable resolved
dependencies and pnpm lock do not change the V1 npm lock. Then:

```sh
LIST_RANDOMIZER_WIRE=target/wire.json cargo test --locked emit_renderer_wire
node scripts/renderer-check.mjs
```

The renderer script uses TypeScript's AST to extract and execute the actual
functions from the byte-exact beta renderer fixtures. Wire data comes from the
shipping adapter using actual SDK operations. It does not simulate a full React
mount, debounced browser input, global keyboard handling, OS clipboard, host
installation, or dispatch registration. Original shortcuts are present in the
wire metadata; desktop interaction still requires host QA.

Coverage includes duplicate/blank/comma/Unicode multisets, original shuffle loop
boundaries, UTF-8 replacement decoding, copy separator, live-versus-restart comma
semantics, missing/empty/cleared history, cross-instance stale writes/copy,
refresh/retry, stable queued field IDs, disposal/relaunch rejection, corruption,
invalid command/action/operation, transport/error-envelope clipboard failures,
actual unwritable-directory atomic write failure, and unrelated-key/V1 isolation.
The lock pins WASI transitive packages below newer releases requiring Rust 1.87.
All build output and disposable dependency installs stay under ignored `target/`.
