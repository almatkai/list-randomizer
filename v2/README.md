# List Randomizer — Native V2

Standalone Rust library and concrete compiled-in Tezbar adapter for the `index`
command, originally by **quuu**. V1 remains in the repository root. This directory
requires host registration and a Tezbar rebuild; its JSON manifest cannot load Rust
code dynamically. macOS and Windows desktop verification are both **not-run**.

## Behavior and deliberate adaptations

Items retain duplicates, whitespace, Unicode and literal commas. Add ignores only
the exact empty string. Copy List sends `items.join(", ")` through the real SDK
clipboard route. Randomize uses the descending Durstenfeld/Fisher–Yates loop with
a fresh Rust RNG sample per swap, matching the original Math.random algorithm;
this feature makes no cryptographic guarantee. Row actions retain Add, Randomize
(`cmd+j`), Copy List (`cmd+l`) and Clear List (`ctrl+x`). Add is also a root action
when empty, reachable through the host Actions menu.

**Add opens a Form, then its Add action appends one item.** Unlike V1's list search
draft, the list search here is inert and never filters items. The beta renderer
sets `__hasServerSearch` for the original React `onSearchTextChange` handler, so
V1 also bypasses client filtering in this host. Native List supports a controlled
`searchText`, but debounces changes for 200 ms without an input-cycle identifier;
the beta V1 shim's `clearSearchBar` is a no-op, and the native SDK has no such
RPC. A delayed search callback cannot be reliably distinguished from new input
after Add/Clear. The Form adaptation uses real `Form.TextField` properties
`id`, `value`, `actionId`, `formValues.value` for changes and `formValues[id]` on
submit. No new host fields or RPCs are required. Field and change-action IDs stay
stable while typing and through an unchanged heartbeat. Add, Back, mutations,
remote revisions, disposal and relaunch invalidate the previous input cycle.
Back retains the draft. Empty Add does nothing. Storage/clipboard errors leave
the view and draft retryable; no successful mutation view is committed first.

### History format

The SDK's own `native-extensions-v2/list-randomizer/storage.json` namespace contains
only one owned key, `listRandomizerHistory`:

```json
{"schema":1,"revision":"0123456789abcdef0123456789abcdef","content":"one,two"}
```

This wraps the **original comma text**, rather than upgrading it to a JSON item
array. Live `"a,b"` remains one item until restart or a new revision from another
instance, when `.split(",")` yields two. `content:""` restores `[""]`; an absent
key or `content:null` restores `[]`. Original file reads used UTF-8 replacement
decoding; `decode_history` documents/tests that behavior. Native storage contains
valid JSON text and does not read or migrate installed V1 profiles or files.

Source V1 Clear List trashes its physical history file. V2 atomically sets only
this key's content to `null`, retaining a new revision tombstone. It never trashes
a supplied path or another key, and preserves all unrelated SDK keys. This
scoped adaptation keeps clear distinct from an empty history file and prevents
an old instance from resurrecting a cleared list.

Each list mutation checks the full observed snapshot inside SDK `update_store`,
then atomically persists before committing session state. Host sessions already
serialize their own state; an adapter lane serializes independent instances
through reads, mutations and clipboard effects. A stale instance fails with
“refresh and retry”, rather than overwriting another list. Server refresh reads
the latest store, retains a local draft, and rotates IDs when the revision changes.
Malformed JSON, wrong schemas and wrong types fail without resetting storage.
The real SDK lane coordinates **one host process**, not multiple application
processes. If a host cancellation happens after a blocking SDK write is launched,
that write may finish; refresh recovers its revision and invalidates old actions.

## Host integration (beta API reference)

Reference commit: `fd300e276356587726566026f072563eb9c34cb1`. The app is not modified
by this package. To integrate in a host checkout:

1. Vendor this directory at the stable path
   `src-tauri/native-extensions/list-randomizer/`.
2. Add `list-randomizer-v2 = { path = "native-extensions/list-randomizer" }` to
   `src-tauri/Cargo.toml` dependencies. Its lock pins Rust 1.85-compatible versions;
   resolve the host lock deliberately when adding it.
3. In `src-tauri/src/native_core/extensions/mod.rs`, declare:

   ```rust,ignore
   #[path = "../../../native-extensions/list-randomizer/tezbar_adapter.rs"]
   mod v2_list_randomizer;
   ```

   This places the adapter beside the actual `v2_sdk` module. Its `handle` has
   the unchanged beta `CoreContext/command/operation/payload/state` signature.
4. Register `list-randomizer` in `v2_runtime::{slug,commands,handle}` using
   `v2_list_randomizer::{commands,handle}`. Append `extension.json` as an entry in
   `v2_catalog.json`; the beta registry/catalog use this compiled dispatch to
   derive runtime coverage and install metadata. Keep the replaced V1 package.
5. Copy **unchanged** `assets/command-icon.png` to
   `src/renderer/public/extensions/icons/list-randomizer.png`. The manifest URL
   is `/extensions/icons/list-randomizer.png` (not an app-root `public` path).
6. Rebuild Tezbar, install through its Native V2 catalog, and run real desktop
   QA separately on macOS and Windows. Update verification only with that evidence.

No filesystem helper, file export, dynamic loader or legacy profile access is
needed. `tezbar_adapter.rs` compiles unchanged against the test fixtures' actual
beta `common.rs` and `v2_sdk.rs`; only the desktop event emitter and IPC router
are stubbed, including OS clipboard. The tests also run the SDK's own tests.

## Validation and licensing

See [VALIDATION.md](VALIDATION.md) for commands, results and limits. `cargo package`
includes the adapter, fixtures, oracle, icon, manifest, lock and licenses; tests
run from an extracted archive without the host checkout. Fixtures are byte-exact
and hashes are in [PROVENANCE.json](PROVENANCE.json). The JavaScript oracle
executes the original pinned TypeScript shuffle after removing its one type
annotation, using recorded random tapes.

Original source/icon and this port: MIT; original author quuu, Raycast repository
copyright 2021 Raycast, Tezbar port copyright 2026 almatkai. See
[LICENSE-MIT](LICENSE-MIT). Beta SDK/common, renderer and schema fixtures are
Apache-2.0 with their unmodified [license](tests/fixtures/beta/LICENSE). The
combined crate package declares `MIT AND Apache-2.0`. The original Raycast
manifest/license are retained under `tests/fixtures/original/`.
