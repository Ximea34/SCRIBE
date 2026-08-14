# Build prompt — DISCUS (IVAO flight strip manager, Tauri 2 + Vite/React/TypeScript)

> Paste this whole file as the first message of a fresh Opus 5 session (extra high thinking).
> Attach `AURORA-PROTOCOL.md` (French field notes) alongside it.

---

## 0. How I want you to work

1. **Read everything below before writing a single line of code.**
2. **Plan first.** Produce a short architecture plan (module tree, data flow, polling strategy, state ownership, re-render strategy) and wait for my approval before implementing.
3. **Ask before assuming.** Section 12 lists what is still open. Do not guess on those — ask me. If anything else is ambiguous, ask rather than invent.
4. **Never invent protocol behaviour.** The Aurora protocol reference in section 4 is the only source of truth. The official IVAO documentation is known to be wrong (see 4.8). If a field or command is not in section 4, ask.
5. Implement in reviewable increments: (a) Rust protocol layer + tests + mock server, (b) airport config loader + domain/classification layer + tests, (c) IPC + typed bindings, (d) UI shell (titlebar, tabs, responsive grid), (e) VIGIE page, (f) activation modal stub.
6. After each increment: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `tsc --noEmit`, `eslint`. All must pass clean.

---

## 1. Product context

**DISCUS** is a desktop companion for IVAO air traffic controllers working live events in a physical control room. Controllers there still use **paper flight strips**; DISCUS is what they look at to know which strips exist, which are live, and (later) what to print.

- The app is an existing real tool being rebuilt from scratch against a new Figma design.
- Target printer for a later phase: **Zebra ZD410** (direct thermal, ZPL II).
- Reference implementation for the Aurora integration + printing concept: `sslazio1900/AuroraFlightStripPrinter` (C#/.NET). Useful for *what data controllers want on a strip*; its Aurora client library is closed-source, so nothing can be copied — the protocol layer must be written from scratch from section 4.
- Data comes from **IVAO Aurora**, the controller client, over its local 3rd-party TCP socket.
- **Single user.** Exactly one controller uses one instance at a time. No multi-user sync, no shared board, no conflict resolution. Enforce single-instance at the OS level.

The app has four tabs: **VIGIE**, **IFR**, **EDITEUR**, **OPTIONS**.
**Only VIGIE is in scope for this build.** The other three must exist as empty, correctly-styled, routable tab targets — nothing more. Do not design or implement their content.

---

## 2. Stack and hard constraints

**Stack (non-negotiable):**
- **Tauri 2.x**, Rust backend. Async runtime: `tokio`.
- **Vite + React + TypeScript** on the front end.
- **Tailwind (if needed), UI component library (if needed), CSS framework (if needed), state-management library (if needed).** Plain CSS (CSS Modules — natively supported by Vite, dependency if needed).
- Front-end runtime dependencies limited to `react`, `react-dom`, `@tauri-apps/api` (+ Tauri plugins where justified). Justify any addition before adding it.

**Hard rules:**
- **Everything in English** — identifiers, types, comments, commit messages, log messages, docs, file names, CSS class names. The UI *strings shown to the controller* stay in French exactly as in the Figma (`ÉVEILLÉS`, `ACTIVÉS`, `TRANSITS`, `VIGIE`, `IFR`, `EDITEUR`, `OPTIONS`) — treat those as data, kept in one `labels.ts`, not scattered through JSX.
- **Comments: single line maximum, and only where the code cannot speak for itself.** No block comments, no doc-novels, no commented-out code. Rust public API items may carry a one-line `///`.
- **No file over 500 lines.** If a file approaches the limit, split it along a real seam, not arbitrarily.
- **Optimisation is the guiding principle.** See section 9 for the concrete budgets.
- No `unwrap()` / `expect()` / `panic!()` on any path reachable at runtime (tests and `main` startup excepted). Errors are typed (`thiserror`) and propagated.
- No `any` in TypeScript. `strict: true`, `noUncheckedIndexedAccess: true`.

---

## 3. Architecture

**The Rust side owns the domain state. React is a thin renderer.**

Rationale: parsing, polling, geo maths, sorting and classification are all cheap in Rust and expensive to redo in JS on every tick; and one source of truth removes a whole class of desync bugs. React holds a mirror of the state solely to render it.

```
Aurora (TCP 127.0.0.1:1130)
        │  line protocol, ASCII, CRLF
        ▼
  aurora::client        connection, per-command FIFO, timeouts, auto-reconnect
        │  parsed typed responses
        ▼
  aurora::scheduler     polling budget, per-callsign refresh priority
        │
        ▼
  domain::store         Flight registry + strip state machine (source of truth)
        │  minimal diffs, coalesced ≤10 Hz, columns already sorted
        ▼
  ipc::events           Tauri events → front-end
        │
        ▼
  external store (TS)   useSyncExternalStore + selectors → React
```

**Front-end → back-end** goes through Tauri commands (`invoke`), never the other way for state mutation.

**Type sharing:** generate the TypeScript types from the Rust types — use `specta` + `tauri-specta` to emit `src/types/bindings.ts` at build time. Hand-maintained duplicate types are not acceptable.

### React state strategy (this matters as much as the Rust side)

- A **module-level external store** in `src/app/store.ts`, fed by the Tauri event listener, exposed to React through **`useSyncExternalStore` with per-slice selectors**. Do **not** put high-frequency traffic data in React Context — every consumer would re-render on every tick.
- Components subscribe to the narrowest slice they need: a `Strip` subscribes to its own callsign's data, a `Column` subscribes only to its ordered list of callsigns (an array of strings), not to the flight objects.
- `Strip` is `React.memo`-ised with a `key` of the callsign. Re-rendering a column must not re-render unchanged strips.
- No inline object/array/function literals passed as props into memoised components.
- Non-urgent bulk updates go through `startTransition`.
- React 19 (or 18.3+). You may enable the React Compiler if it does not fight the manual memoisation — tell me either way.

### Suggested module tree (adapt if you have a better seam, but justify)

```
src-tauri/src/
  main.rs                  thin
  lib.rs                   app builder, plugin + state wiring
  error.rs                 AppError, thiserror
  settings.rs              persisted config model + load/save
  airports/
    mod.rs                 Airport model + registry
    parser.rs              config file format (see 5.5)
    coords.rs              DMS / decimal coordinate parsing
  aurora/
    mod.rs
    codec.rs               CRLF line framing, ASCII, field splitting
    protocol.rs            Command enum, Response enum, parsers
    types.rs               FlightPlan, TrafficPosition, ...
    client.rs              TCP task, per-command FIFO queues, timeouts
    scheduler.rs           refresh budget + priority queue
  domain/
    mod.rs
    flight.rs              Flight aggregate (fp + position + strip state)
    strip.rs               StripState enum + transitions
    classifier.rs          column assignment rules
    ordering.rs            EOBT / distance / altitude sort keys
    geo.rs                 haversine, nm conversion
    store.rs               registry, diff computation
  ipc/
    mod.rs  commands.rs  events.rs
src/
  main.tsx
  App.tsx
  app/
    store.ts               external store + selectors
    ipc.ts                 typed invoke/listen wrappers
    router.ts              tab switching
    labels.ts              French UI strings
  hooks/
    useStoreSlice.ts  useAuroraStatus.ts
  ui/
    Titlebar.tsx  Tabs.tsx
    components/Strip.tsx  Column.tsx  StripList.tsx  Modal.tsx
    pages/VigiePage.tsx  IfrPage.tsx  EditorPage.tsx  OptionsPage.tsx
  styles/
    tokens.css  base.css  *.module.css
  types/bindings.ts        generated
```

---

## 4. Aurora 3rd-party protocol — authoritative reference

This is the English translation of the attached field notes, gathered from real sessions. **Treat this, not IVAO's official documentation, as correct.**

### 4.1 Enabling and connecting

- The controller must enable it in Aurora: **F7 → Other → 3rd Party Software Access**. Without it nothing listens and connection fails immediately.
- Aurora listens **locally only**: `127.0.0.1:1130`, TCP. Plain socket client, **no authentication**.
- **ASCII**. Every message, request or response, is one line terminated by `\r\n`.
- Line format: `#COMMAND;field1;field2;...` — `;`-separated, empty fields allowed (`;;`).
- **No request/response correlation ID.** A response is matched to a request by the command name at the head of the line. Multiple in-flight requests of the same command are assumed to answer in send order → **one FIFO queue per command name**.
- **No keepalive/heartbeat.** Disconnection is detected only via the TCP socket close event.

### 4.2 Commands used by this project

| Command | Meaning | Response |
|---|---|---|
| `#CONN` | Which station am I (the controller connected to Aurora) | `#CONN;CALLSIGN` |
| `#SELTFC` | Callsign currently selected in Aurora | `#SELTFC;CALLSIGN` (empty if none) |
| `#FP;CALLSIGN` | Filed flight plan | fields in 4.3 |
| `#TRPOS;CALLSIGN` | Real-time position/state | fields in 4.4 |
| `#TR` | Raw list of callsigns currently visible on radar | list of callsigns |
| `#TRPATHL` / `#TRPATHA` | Expanded remaining route with ETO | not used — see 11 |
| `#ATC` | Online ATC positions | repeated `STATION:FREQ` |

`%SELTFC%` can replace a callsign in any of these — but see the traps in 4.6. **Default policy for this project: never use `%SELTFC%` as a substitution argument.** `#SELTFC` on its own is safe and is the only way to read the current Aurora selection.

### 4.3 `#FP` fields (verified against real data, 2026-07-22)

```
0  callsign               8  flightType   (S/N/G/M/X)
1  dep                    9  equipment
2  arr                    10 cruiseLevel  (e.g. "F330")
3  alternate              11 cruiseSpeed  (e.g. "N0450")
4  eobt                   12 endurance
5  aircraft (ICAO type)   13 eet
6  wake (L/M/H/J...)      14 route (free text as filed)
7  rules (I/V/Y/Z)        15 remarks
```

⚠️ The official documentation **swaps fields 7 and 8**. Verified wrong on real data. Use the table above.

### 4.4 `#TRPOS` fields

```
0  callsign                        11 spdLabel
1  heading (deg)                   12 assumedBy    (station that assumed the traffic; empty if none)
2  track (deg)                     13 nextStation
3  altitude (ft)                   14 onGround     ("1"/"0")
4  groundSpeed (kt, GROUND speed)  15 isSelected   ("1"/"0")
5  lat                             16 wasSelected  ("1"/"0")
6  lon                             17 gate         (current stand — see 5.4)
7  squawkSet                       18 voice
8  squawkLabel                     19 (undocumented, meaning unknown — do not use)
9  wpLabel                         20 verticalSpeed (ft/min, >0 climb, <0 descent) — undocumented but confirmed
10 altLabel                        21 assignedGate  (assigned stand, distinct from 17) — undocumented, NOT verified
```

⚠️ Official docs stop at field 18. Fields 19–21 are absent from it entirely. Field 20 was confirmed by cross-checking real data. Field 21 is documented but unverified — do not build on it without a real capture.

⚠️ **Field 17 caveat (flagged as unverified in the source notes):** Aurora appears to populate `gate` **only for the airport whose stand plan is loaded / actively controlled** by the connected position. On an airport the position does not control, it stays empty even when the aircraft is visibly parked (`onGround` = 1). Our controlled airport is exactly that case, so it should work — but this drives arrival removal (5.4), so verify it against a real capture and implement the fallback described there.

### 4.5 `#TRPATHL` / `#TRPATHA` (documented, not used)

`#TRPATHx;CALLSIGN;FIX1:ETO1;FIX2:ETO2;...`. Aurora expands SID/STAR/airways itself into named points. `ETO` is `-` for an already-overflown point.

⚠️ Aurora sometimes emits an **unnamed turning point** (empty/blank fix) mid-sequence, typically just before an airport, which breaks naive "last point" logic. Recorded here for completeness. **This build does not use these commands** — arrival ordering is derived from `#TRPOS` alone (5.3), which avoids the trap entirely and removes a whole polling stream. Do not reintroduce them without asking.

### 4.6 Traps observed in real sessions (none documented officially)

1. **`#TRPOS;%SELTFC%` closes the socket** if nothing is selected in Aurora at that moment. Guard: check `#SELTFC` alone first (harmless), or simply never use the substitution. This project chooses the latter.
2. **`%SELTFC%` is not required to query a specific aircraft.** `#FP;CALLSIGN`, `#TRPOS;CALLSIGN` work fine on an aircraft that is not selected.
3. **`wpLabel` (`#TRPOS` field 9)** is either a procedure label — `"SIDNAME RUNWAY"`, always two space-separated words, e.g. `"BODRU8A 04R"` — or, when a controller has cleared the traffic direct to a point, the bare point name with no space, e.g. `"MTL"`. **This is the only observable signal of a manually issued direct.**
4. **Fields beyond the documented range exist and carry real data.** Always verify an unknown field against a real sample before dismissing it.

### 4.7 Errors, and the `@` prefix

- Refusals come back as `@ERR;#COMMAND;ARGUMENT;reason` (prefix `@`, sometimes `$`). The first field after `@ERR` names the offending command — use it to fail the *correct* pending request rather than the oldest in the queue.
- The `@` prefix is **not exclusive to errors**: an empty baylist answers `@BAY;No data in bay` instead of `#BAY;...`. So `@` also marks "empty/informational" responses. The parser must handle `#`, `@` and `$` prefixes and must not assume a `#` echo.

### 4.8 Reliability of the official documentation

The official IVAO documentation has already been caught with: 2 swapped fields, several undocumented fields carrying real data, and a major operational constraint (`#TRPOS;%SELTFC%` killing the socket) absent from every document. **Never trust the doc alone.** Any new field or command must be checked against a real capture before code depends on it.

### 4.9 Implementation requirements for the protocol layer

- A pure, side-effect-free parser (`aurora::protocol`) that maps a raw line → typed `Response`, fully unit-tested including: empty fields, trailing separators, `@ERR`, `@BAY`, unknown commands, malformed lines, truncated lines, oversized lines.
- Framing (`aurora::codec`) must handle split/coalesced TCP reads, lone `\n`, and cap line length to avoid unbounded buffering.
- `client.rs`: single owning task, per-command FIFO of pending requests, per-request timeout (configurable, default ~2 s), automatic reconnect with capped exponential backoff, clean `close` detection, and a connection state channel.
- **Write a mock Aurora server** (`src-tauri/tests/mock_aurora.rs` or a small `mock-aurora` bin) that replays captured samples and can be scripted to reproduce every trap above, including the socket-close case. Integration tests run against it. This is the tool that makes the rest testable without Aurora running.
- Never assume Aurora is reachable. Never block the UI on it.

### 4.10 Connection lifecycle

**Auto-connect on launch**, then retry silently on failure with capped exponential backoff, indefinitely. No connect/disconnect UI anywhere in this build — connection management will live in the OPTIONS tab later. Expose connection state in the store and log every transition; just do not surface it in VIGIE.

---

## 5. Domain model and classification

### 5.1 Entities and the FPL rule

- `Flight` = `callsign` + `FlightPlan` + optional `TrafficPosition` + `StripState` + timestamps (`first_seen`, `last_seen`, `fp_fetched_at`).
- **Traffic with no flight plan is never displayed, in any column.** No FP → not on the board at all. Keep it out of the registry entirely (or in a clearly separate "pending FP" holding area, since `#FP` is fetched asynchronously after `#TR` reveals a new callsign — a flight must not flicker onto the board before its FP resolves, nor be discarded permanently just because the first `#FP` attempt failed). Decide which, and say why.
- `StripState`: `Awake` → `ActivatedDeparture`, plus `Arrival`, `Transit`, and a terminal/archived state. Make transitions explicit and unit-tested; illegal transitions must be unrepresentable or rejected.

### 5.2 Reference values

Let `ctrl_airport` = the ICAO of the controlled airport, `centre` = its lat/lon, `field_elev` = its elevation in feet — all three from the airport config file (5.5). Radius `R` = **20 NM** (configurable, default 20).

Distances are great-circle (haversine) on `#TRPOS` lat/lon (fields 5/6), in NM.

### 5.3 Column assignment and ordering (VIGIE)

| Column | Rule | Ordering |
|---|---|---|
| **ÉVEILLÉS** (left) | Has FP, on radar (`#TR`), `fp.dep == ctrl_airport`, not yet activated. | **EOBT** ascending |
| **ACTIVÉS — departures** (middle, top) | An ÉVEILLÉS strip the controller explicitly activated through the modal. | **EOBT** ascending |
| **ACTIVÉS — arrivals** (middle, bottom) | Has FP, `fp.arr == ctrl_airport`, **within `R` of `centre`**. Entered **automatically** — arrivals are never "activated". | **distance ascending, then altitude ascending** |
| **TRANSITS** (right) | Has FP, within `R` of `centre`, and in none of the above. | **distance to `centre` ascending** |

**Activation only ever applies to departures from `ctrl_airport`.** It follows that a transit can never be activated — by construction it is not in ÉVEILLÉS (`fp.dep != ctrl_airport`). Encode this as a type-level or state-machine invariant, not a runtime check buried in the UI.

**Arrival ordering — closest first, lowest first.** Primary key: distance to `centre`. Tie-break: altitude. My reading of "les plus proches, les plus bas (sans toucher le sol)" is that ground traffic stays listed until parked (5.4) and naturally sorts to the top, since it is both nearest and lowest. Implement it that way and flag it back to me if you think it reads otherwise. Compute height above field as `altitude - field_elev` for display purposes; it does not change the ordering (constant offset).

> Note: this **supersedes** an earlier instruction to sort arrivals by ETA from `#TRPATHL`. Distance is a good ETA proxy inside 20 NM, costs nothing extra (`#TRPOS` is already polled) and sidesteps the blank-fix trap of 4.5.

**EOBT parsing:** `#FP` field 4, `HHMM` UTC. Handle the midnight wrap when comparing against the current time. Missing or unparseable EOBT sorts last, deterministically (never a non-total ordering, never a comparator that can panic).

Classification and ordering must be **pure functions** of `(Flight, AirportConfig, Settings)` in `domain::classifier` / `domain::ordering`, exhaustively unit-tested — including: no position, stale position, no EOBT, equal distances, an aircraft exactly on the 20 NM boundary, and one that is both inside the ring and to/from the controlled airport.

### 5.4 Removal rules

Specific and deliberate — do not replace them with a generic timeout:

- **Departures** leave the board once they are **airborne and outside the ring**: `onGround == 0` **and** `distance(position, centre) > R`. `onGround == 0` alone is a sufficient definition of airborne — no altitude or groundspeed threshold needed, since the distance condition already rules out flicker on rotation.
- **Arrivals** leave the board once **parked**: `#TRPOS` field 17 (`gate`) is non-empty. Given the 4.4 caveat on that field, implement a configurable fallback — e.g. `onGround == 1` and `groundSpeed` below a few knots, sustained for N seconds — and log which of the two triggered, so we can validate field 17 in the field.
- **Radar dropouts** are separate: a flight disappearing from `#TR` must not vanish instantly (dropouts happen). Apply a configurable grace period, then archive.
- Activated departures must **survive an app restart and an Aurora reconnect** — this is a live-event tool, a crash must not wipe the controller's board. Persist activation state keyed by callsign, with a sane staleness cutoff.

### 5.5 Airport config file

The centre point is **not available anywhere in the protocol** — it comes from an external airport config file supplied by the controller. The controller will pick the file and the active airport from the **OPTIONS page, which does not exist yet**. For this build, store the file path and selected ICAO in persisted settings, loadable at startup, with a developer-facing way to set them (CLI flag, env var, or hand-edited settings file — your call, keep it out of the VIGIE UI).

**Format** — plain UTF-8 text, one airport per line, `;`-separated (mirrors Aurora's own convention), `#` comments, blank lines ignored:

```
# DISCUS airport configuration
# ICAO;NAME;LAT;LON;ELEV_FT
LFLL;LYON SAINT EXUPERY;45°43'32"N;005°04'52"E;821
```

**Coordinate parsing** must be tolerant, and lives in its own module (`airports/coords.rs`) with its own tests. Accept at minimum:

- DMS with symbols: `45°43'32"N`, `005°04'52"E`
- DMS in ASCII, hemisphere as prefix or suffix, space- or dot-separated: `N45 43 32`, `N045.43.32`, `45 43 32 N`
- Decimal degrees, signed or hemisphere-suffixed: `45.725556`, `-5.081111`, `5.081111E`
- Fractional seconds

Normalise everything to `f64` decimal degrees, north/east positive. **Test fixture:** the LFLL line above must parse to lat `45.725556`, lon `5.081111` (tolerance 1e-6), elevation `821`.

**Parser rules:** trim whitespace; tolerate a UTF-8 BOM; uppercase and validate the ICAO against `^[A-Z]{4}$`; validate lat ∈ [-90, 90] and lon ∈ [-180, 180]; elevation is an integer in feet and may be negative. Ignore any extra trailing columns (reserved for future fields) and log them at debug level. A malformed line is skipped with a logged error naming the line number — **it must not abort the whole file**. Duplicate ICAO: last wins, with a warning. If the selected ICAO is absent from the file, that is a hard, clearly-reported startup error.

---

## 6. Aurora polling strategy (this is where the performance is won)

The protocol has **no push and no subscription** — everything is polled, one aircraft at a time. A naive "query everything every tick" loop does not survive a busy event. Design `aurora::scheduler` deliberately:

- `#CONN` once on connect, then rarely (e.g. every 30 s) to detect a station change.
- `#TR` on a fixed cadence (~1 s) → authoritative set of visible callsigns; drives add/remove.
- `#FP` **once per newly seen callsign**, then cached. Flight plans change rarely (amendments) — refresh on a long TTL (~60 s), never every tick. A callsign whose `#FP` comes back empty is not displayed (5.1); retry it a bounded number of times before giving up, and re-arm on reconnect.
- `#TRPOS` is the only high-frequency stream and the expensive one: one request per aircraft. Use a **request budget** (configurable, e.g. max N requests/second) and a **priority queue** rather than a blind full sweep:
  - anything on the board — activated departures, arrivals, transits, éveillés — refreshes fastest,
  - traffic outside the ring and irrelevant to any column round-robins on a much slower TTL, purely to detect it entering the ring,
  - a full sweep must still complete within a bounded worst case (target: every aircraft refreshed at least every 3–4 s at 200 aircraft).
- Coalesce: never have two identical pending requests for the same callsign.
- Back off automatically on repeated timeouts or `@ERR` for a given callsign.
- Emit state diffs to the front end **coalesced at ~10 Hz max**, never one event per parsed line. Classification and sorting happen in Rust; the front end receives already-ordered callsign lists.

Make every cadence, TTL and budget a named constant in one place, and expose the important ones in settings.

---

## 7. VIGIE UI — Figma spec

Design: `SCRIBE - Vigie`, file `sc7YTvdz9riCv38KY5MD6y`, node `57:69`. The Figma canvas is **1920 × 1080**, but that is the reference composition, **not a fixed target** — see 7.1.

### 7.1 Responsive requirements

The layout must be **genuinely responsive**, not a scaled screenshot.

- **Minimum supported viewport: 1280 × 720** (tablet class). Below that, degradation is acceptable but the app must not break.
- At 1920 × 1080 the result should match the Figma pixel for pixel.
- **Horizontal:** three equal columns via CSS Grid (`repeat(3, 1fr)`), gaps and outer margins proportional to the reference (23 / 1894 ≈ 1.21 % gap, 13 px margins at reference width).
- **Vertical:** the ACTIVÉS column splits into two equal panes (464 + 18 + 464 = 946) → `grid-template-rows: 1fr 1fr` with a proportional gap.
- **Typography and strip height** scale with a bounded factor so text stays legible at 720p and does not become absurd on a 4K panel:
  ```css
  :root { --s: clamp(0.667, min(100vw / 1920, 100vh / 1080), 1.5); }
  ```
  At 1280 × 720 that yields ≈ 0.667 → 24 px strip text, 57 px strips. Verify legibility at that size and tell me if the floor should be higher.
- **Overflow is the normal case**, not an edge case: at 720p a pane fits roughly nine strips. Panes must scroll independently, with scrollbars styled to match the design (not the OS default). Above a configurable threshold, virtualise the list. Decide and justify the threshold.
- No mobile/portrait layout. No breakpoints that reorder or stack the columns — three columns always.

### 7.2 Colours

| Token | Value | Used for |
|---|---|---|
| `--c-titlebar` | `#1B2938` | top bar background |
| `--c-window` | `#303030` | window frame, 1px solid white border |
| `--c-bg` | `#1B1C1B` | content background, column bodies, **active** tab |
| `--c-tab` | `#679BD2` | inactive tabs (IFR, EDITEUR, OPTIONS) |
| `--c-header` | `#333333` | column header background |
| `--c-header-text` | `#A1BEDB` | column header text |
| `--c-border` | `#838783` | column body border, 2px solid |
| `--c-strip` | `#D9D9D9` | strip background |
| `--c-strip-text` | `#000000` | strip text |
| `--c-text` | `#FFFFFF` | tab text |

### 7.3 Typography

**Inria Sans, Bold**, everywhere. **Bundle the font locally as woff2** with `@font-face` — control rooms are not guaranteed online and a font swap mid-session is unacceptable.

| Element | Size @1920 | Tracking | Colour |
|---|---|---|---|
| Tab label | 24 px | normal | `#FFFFFF` |
| Column header | 36 px | 0.1em (3.6 px) | `#A1BEDB` |
| Strip text | 36 px | 0.1em (3.6 px) | `#000000` |

### 7.4 Reference geometry (px at 1920 × 1080)

Use these to derive the proportions; they are the spec at reference size.

**Title bar** — full width × 42, `--c-titlebar`. Must be the Tauri drag region (`decorations: false`, custom controls).

**Tabs** — height 34, top 8 (flush with the bottom of the 42 px bar), radius **16 px on the top corners only**, label centred.

| Tab | x | width |
|---|---|---|
| VIGIE | 0 | 227 |
| IFR | 248 | 227 |
| EDITEUR | 488 | 227 |
| OPTIONS | 1656 | 119 (height 33, top 9) |

Active tab uses `--c-bg` so it reads as continuous with the content area; inactive tabs use `--c-tab`. VIGIE/IFR/EDITEUR are a left-anchored group; OPTIONS is right-anchored. (The inter-tab gaps in the Figma are inconsistent — 21 px then 13 px. Normalise to a single value and tell me which you picked.)

**Window controls** — top-right: minimize 24 × 24 @ (1808, 9), maximize 20 × 20 @ (1849, 11), close 24 × 24 @ (1885, 9). Wire to the Tauri window API. Export the three icons from the Figma node.

**Content area** — below the title bar, `--c-bg`.

**Columns container** — inset 13 px left / right / bottom, 13 px below the title bar. Three columns of 616 with 23 px gaps at reference width.

**Column header** — full column width × 53, `--c-header`, label centred: `ÉVEILLÉS`, `ACTIVÉS`, `TRANSITS`.

**Column body** — `--c-bg`, 2 px solid `--c-border`. Left and right: one pane, 946 tall. Middle: two panes of 464 with an 18 px gap — departures top, arrivals bottom.

**Strip** — 592 × 86, `--c-strip`, inset 12 px left/right inside the pane (12 + 592 + 12 = 616 ✓). First strip top = pane top + 14. Vertical gap between strips: 18 px.

**Strip content** — four centred text cells, vertical centre at 43 px. Horizontal centres relative to the strip's left edge (express as percentages of strip width for the responsive layout):

| Cell | centre x | as % of 592 | content |
|---|---|---|---|
| Callsign | 116 | 19.6 % | `#FP` field 0 |
| ADEP | 295.5 | 49.9 % | `#FP` field 1 |
| ADES | 408.5 | 69.0 % | `#FP` field 2 |
| Rules | 531 | 89.7 % | `#FP` field 7 (`I`/`V`/`Y`/`Z`) |

**Overflow marker:** any value too long for its cell is truncated and suffixed with **`>`** — not an ellipsis, not a wrap, not a shrink-to-fit. Never let text bleed into an adjacent cell or out of the strip. Implement truncation as a shared helper so the same rule applies anywhere long values appear (strip cells, and the route field in the modal).

### 7.5 Interaction

- Clicking a strip in **ÉVEILLÉS** opens the **activation modal**. Confirming moves it to the ACTIVÉS departures pane.
- **The modal's design does not exist yet — I will send you the Figma later.** For now: build a *deliberately minimal, unstyled-but-functional* stub behind a clean component seam (`Modal.tsx` + an `ActivationDialog` owning only content). Do not invent a visual design for it, do not spend effort polishing it, and keep the seam narrow enough that dropping in the real design touches one file.
- The stub shows the flight data available (callsign, type, wake, rules, ADEP/ADES/alternate, EOBT, RFL, route, squawk, assumed-by, stand) and has confirm and cancel actions.
- Keyboard: `Esc` cancels, `Enter` confirms, focus trapped while open, focus restored on close.
- Arrivals and transits are **not** clickable-to-activate. Make that obvious in the interaction affordance, not just in code.
- The board must be usable with a mouse at arm's length on a shared screen — hit targets are already large, keep them large at 720p too.

---

## 8. Tauri specifics

- `decorations: false`, custom titlebar, drag region, working minimize/maximize/close, resizable, minimum window size **1280 × 720**.
- Single instance enforced (`tauri-plugin-single-instance`) — one controller, one board.
- Tauri 2 **capabilities**: grant only what is used. Justify each permission. The Rust side does the TCP — the front end gets no network capability.
- Strict CSP. No remote assets at runtime: fonts, icons and everything else bundled.
- Settings persisted in the OS app-config dir (`tauri-plugin-store` or a plain serde JSON file — your call, justify it). Include at minimum: Aurora host/port, airport config file path, selected ICAO, ring radius (default 20 NM), polling budgets/cadences, arrival-parked fallback thresholds, radar-dropout grace period.
- Structured logging via `tracing`, with a file target so a controller can send a log after an event. No `println!`.
- Graceful shutdown: the TCP task must be cancelled and the socket closed cleanly on window close.

---

## 9. Performance budgets

Acceptance criteria, not aspirations:

- **200 simultaneous traffics** with no visible degradation.
- Idle CPU (connected, ~50 traffics, no interaction): **< 2 %** on a modern laptop.
- Resident memory: **< 150 MB**.
- A store update must not re-render an unchanged strip. Verify with the React Profiler and report the numbers — "it feels fine" is not evidence.
- Any UI update completes within one frame (**< 16 ms**); no layout thrash, no full list rebuild.
- No per-tick allocation churn in the Rust hot path; parse into borrowed slices where possible; no `String` allocation per field for unchanged data.
- Zero polling, zero rendering and zero subscriptions for tabs that are not visible.
- Scrolling a full pane at 720p stays at 60 fps.

---

## 10. Quality bar

- `cargo clippy -- -D warnings` and `tsc --noEmit` clean. `rustfmt` + `prettier` + `eslint` (with `react-hooks` rules) enforced.
- Unit tests: protocol parser (every trap in 4.6/4.7), coordinate parser (every accepted form, plus the LFLL fixture), airport file loader (comments, blank lines, bad lines, duplicates, missing ICAO), classifier, ordering (EOBT wrap, missing values, ties, boundary distance), removal rules, strip state machine, geo/haversine.
- Integration tests against the mock Aurora server, including disconnect/reconnect and the socket-close trap.
- No dead code, no TODOs left behind, no placeholder implementations shipped as if complete (the modal stub is the one declared exception, and must be labelled as such).
- Conventional commits, English.
- A `README.md` (English) with: prerequisites, the Aurora F7 → Other → 3rd Party Software Access step, the airport config file format with the LFLL example, dev commands, build, and how to run the mock server.

---

## 11. Explicitly out of scope for this build

- The IFR, EDITEUR and OPTIONS tab contents (empty styled placeholders only). OPTIONS will later own airport-file selection and Aurora connection management — leave the settings and connection-state APIs ready for it.
- The activation modal's visual design (stub only, see 7.5).
- Any connect/disconnect UI (see 4.10 — auto-connect only).
- Zebra ZD410 printing. **But**: leave a clean seam for it — a `printing` module boundary and a strip-data DTO decoupled from the UI. Note that the ZD410 speaks ZPL II, so the future path is ZPL generation, not the HTML templating the reference C# project uses. Do not implement it now.
- `#TRPATHL`, `#TRPATHA`, `#BAY`, `#LBGTE`, `#ATCT`, gate *assignment*. The protocol layer may model them; nothing in VIGIE depends on them.

---

## 12. Remaining open points — raise these in your plan, do not silently decide them

1. **Pending-FP handling** (5.1): where a callsign lives between `#TR` revealing it and `#FP` resolving, and what happens when `#FP` never resolves.
- We don't show the AC strip
2. **Arrival ordering reading** (5.3): confirm distance-then-altitude with ground traffic sorting to the top.
- YES
3. **Virtualisation threshold** (7.1) and the scale floor at 720p.
- YES
4. **Longest realistic callsign** to size the strip cell before the `>` truncation kicks in.
- YES
5. Anything in section 4 you find you need and that is not documented there — ask, do not test against live Aurora and guess.
- NO

(docs avlb in /docs/**)