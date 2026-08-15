# SCRIBE

Flight strip manager for IVAO air traffic controllers working live events with paper strips.
SCRIBE reads live traffic from the [IVAO Aurora](https://www.ivao.aero/) controller client over its
local 3rd-party TCP socket and shows which strips exist, which are live, and which are transiting.

Desktop app: Tauri 2 (Rust) + Vite / React / TypeScript.

> **Status.** Increments A to E of six are complete. VIGIE shows live strips end to end: the
> Aurora protocol layer, connection client, mock server, airport configuration, domain layer
> (classification, ordering, removal rules, board diffing), polling scheduler, engine, typed IPC,
> interface shell and the board itself. The activation dialog lands next, so strips are still
> read-only.

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

## Airport configuration

The centre point of the controlled airport is not available anywhere in the Aurora protocol, so it
comes from a file you supply. Plain UTF-8 text, one airport per line, `;`-separated, `#` starts a
comment, blank lines are ignored:

```
# SCRIBE airport configuration
# ICAO;NAME;LAT;LON;ELEV_FT
LFLL;LYON SAINT EXUPERY;45°43'32"N;005°04'52"E;821
```

Coordinates are deliberately forgiving. All of these are the same longitude:

```
005°04'52"E      E005 04 52      005 04 52 E      E005.04.52      5.081111E      5.081111
```

DMS with or without symbols, hemisphere before or after, dot- or space- or colon-separated,
signed or hemisphere-suffixed decimal degrees, and fractional seconds. North and east are
positive. Elevation is an integer in feet and may be negative.

A malformed line is skipped and logged with its line number — it never aborts the file. A repeated
ICAO keeps the last definition and warns. Extra trailing columns are reserved for future fields and
ignored. If the airport you selected is not in the file, that is a hard startup error.

Selecting the file and the active airport will live in the OPTIONS tab. Until it exists, both are
persisted in `settings.json` in the OS application config directory, with environment overrides for
development:

```sh
SCRIBE_AIRPORTS_FILE=C:/atc/airports.txt   # path to the file above
SCRIBE_ICAO=LFLL                           # which airport is being controlled
SCRIBE_AURORA_ADDR=127.0.0.1:1130          # where Aurora is listening
SCRIBE_LOG=info,scribe_lib=debug           # tracing filter
```

Without an airport the window still opens and the board stays empty, with the reason in the log —
refusing to start would leave you no way to fix it before OPTIONS exists.

## Polling budget

Aurora has no push and no subscriptions: every aircraft is polled individually. `settings.json`
holds every cadence and budget in one place. The defaults are sized for the worst case in the
specification — 200 simultaneous traffics — and were measured against the mock rather than guessed:

| Setting                   | Default | Why                                                             |
| ------------------------- | ------- | --------------------------------------------------------------- |
| `budgetRequestsPerSecond` | 150     | Measured demand at 200 traffics is ~114/s; the rest is headroom |
| `boardRefreshMs`          | 1000    | Arrivals and transits, whose position decides their order       |
| `nearRefreshMs`           | 2000    | Departures, and traffic within 1.5× the ring                    |
| `farRefreshMs`            | 4000    | Everything else, so it is noticed on approach                   |
| `trafficListIntervalMs`   | 1000    | `#TR` drives every add and remove                               |
| `flightPlanTtlMs`         | 60000   | Amendments are rare; plans are cached, not re-fetched           |
| `emitIntervalMs`          | 100     | Ceiling on how often the front end is told anything             |

Measured at 200 traffics with 127 strips on the board: **113.8 requests/s**, every aircraft reached
at least once every 4.5 s, each flight plan fetched exactly once, and 25 board updates in 9 s
because unchanged boards emit nothing. `budgetRequestsPerSecond` is the one number that still needs
validating against real Aurora — loopback carries it easily, but Aurora's own handler capacity is
unknown. Lowering it degrades gracefully: distant traffic slows down first, the board last.

## Interface

The board is laid out against a 1920 × 1080 reference composition and scales from a 1280 × 720
minimum upward. Two scale factors drive everything, both carrying `px` so a plain number can
multiply them:

```css
--s: clamp(0.667px, min(100vw / 1920, 100vh / 1080), 1.5px); /* the board */
--s-chrome: clamp(0.85px, min(100vw / 1920, 100vh / 1080), 1.25px); /* titlebar and tabs */
```

The board floor puts strip text at 24 px on a 720p panel; the chrome has a higher floor so the
window controls stay comfortably clickable at that size. Three columns, always — there are no
breakpoints that stack or reorder them.

Inria Sans Bold is bundled as woff2 under `src/assets/fonts` (SIL OFL 1.1, licence alongside) and
declared `font-display: block`: a control room is not guaranteed online, and a font swapping in
mid-session would reflow the whole board.

Rust owns the board; React only renders it. Updates arrive as coalesced diffs on one event and
land in a module-level store, never React context. Components subscribe to the narrowest slice
they can: a column to its ordered list of callsigns, a strip to its own callsign alone. So a
reorder never touches a strip, and a strip's data changing never touches its neighbours.

Measured live against the mock at 200 traffics, over a 30-second window: **26 board commits,
zero re-renders of an unchanged strip**, worst commit 0.90 ms, average 0.18 ms. Panes scroll
independently and off-screen strips are skipped with `content-visibility`, so no virtualisation
library is needed.

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
src/
  types/bindings.ts  generated from the Rust types — never edit by hand
src-tauri/src/
  aurora/            protocol parser, line framing, client, polling scheduler
  airports/          configuration loader and coordinate parsing
  domain/            flights, strip state machine, classification, ordering, board diffing
  ipc/               typed Tauri commands and the board event
  printing/          data contract for the future Zebra ZD410 path
  engine.rs          the task that owns the board and drives everything
  settings.rs        every cadence, budget and threshold, in one place
src-tauri/mock-aurora/   mock Aurora server (dev only, never bundled)
src-tauri/tests/     protocol, domain, scheduler and engine tests
docs/                Aurora protocol field notes — the authoritative reference
```

`src/types/bindings.ts` is generated by `tauri-specta` from the Rust command and event types.
`cargo test` regenerates it and fails if the committed copy was stale; to refresh it by hand:

```sh
cd src-tauri && cargo run --bin scribe -- --export-bindings
```

## Protocol reference

`docs/AURORA-PROTOCOL.md` holds field notes gathered from real sessions. **It, not IVAO's official
documentation, is the source of truth for this project**: the official documentation has already
been caught with two swapped `#FP` fields, several undocumented `#TRPOS` fields carrying real data,
and a socket-closing constraint absent from every document. Never add a field or command without
checking it against a real capture first.

## Licence

See [LICENSE](LICENSE).
