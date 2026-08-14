# SCRIBE

Flight strip manager for IVAO air traffic controllers working live events with paper strips.
SCRIBE reads live traffic from the [IVAO Aurora](https://www.ivao.aero/) controller client over its
local 3rd-party TCP socket and shows which strips exist, which are live, and which are transiting.

Desktop app: Tauri 2 (Rust) + Vite / React / TypeScript.

> **Status.** Increment A of six is complete: the Aurora protocol layer, the connection client and
> the mock server. The VIGIE board, the domain layer and the UI land in later increments.

## Prerequisites

- [Rust](https://rustup.rs/) 1.92 or newer
- [Bun](https://bun.sh/) 1.3 or newer
- Windows: the WebView2 runtime (preinstalled on Windows 11)
- [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your platform

## Enabling Aurora's 3rd-party socket

SCRIBE cannot see anything until the controller enables the socket inside Aurora:

**F7 → Other → 3rd Party Software Access**

Aurora then listens on `127.0.0.1:1130` (TCP, local only, no authentication). Without this step
nothing is listening and every connection attempt fails immediately. SCRIBE connects automatically
on launch and retries with capped exponential backoff, so you can enable it at any time.

## Development

```sh
bun install                 # install front-end dependencies
bun run tauri dev           # run the app
```

Quality gates, all of which must pass clean:

```sh
bun run typecheck           # tsc --noEmit, both tsconfigs
bun run lint                # eslint, including react-hooks rules
bun run format:check        # prettier

cd src-tauri
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Mock Aurora server

`mock-aurora` is a standalone stand-in for the Aurora socket. It lets you run and test SCRIBE
without Aurora open and without a controller connected, and it reproduces the protocol traps
recorded in `docs/AURORA-PROTOCOL.md` (socket close on `#TRPOS;%SELTFC%`, `@ERR` refusals, the
`@BAY` informational response, split writes, bare newlines, oversized lines, silent commands).

```sh
cd src-tauri
cargo run -p mock-aurora -- --traffics 200 --port 1130
```

`--traffics` (default 50) generates that many synthetic aircraft around Lyon Saint-Exupéry and
moves them once per second — this is the harness used to verify the performance budgets. `--port`
defaults to 1130, so point it elsewhere if the real Aurora is running.

The same crate is a library, which the integration tests in `src-tauri/tests/` drive in-process on
an ephemeral port.

## Layout

```
src/                 front end (React)
src-tauri/src/
  aurora/            protocol parser, line framing, connection client
  error.rs           typed errors
src-tauri/mock-aurora/   mock Aurora server (dev only, never bundled)
src-tauri/tests/     parser, framer and client integration tests
docs/                Aurora protocol field notes — the authoritative reference
```

## Protocol reference

`docs/AURORA-PROTOCOL.md` holds field notes gathered from real sessions. **It, not IVAO's official
documentation, is the source of truth for this project**: the official documentation has already
been caught with two swapped `#FP` fields, several undocumented `#TRPOS` fields carrying real data,
and a socket-closing constraint absent from every document. Never add a field or command without
checking it against a real capture first.

## Licence

See [LICENSE](LICENSE).
