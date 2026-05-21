# AGENTS.md

Guide for agents (and humans) working in this repository.

## Repository at a glance

Flightradar is a real-time ADS-B aircraft tracker. Two top-level apps:

- `frontend/` — Vue 3 + TypeScript + Vite, served by nginx in production.
- `backend/` — **Python FastAPI** (current production backend).
- `backend-rs/` — **Rust workspace** (in-progress rewrite). On completion it
  will replace `backend/` and the Python tree will be deleted.

Also: `contrib/` (nginx, supervisord, entrypoint), `docs/`, `docker-compose.yml`.

## Migration status

The Rust backend is feature-complete on the
`claude/plan-rust-migration-T0Zet` branch and the Docker image is now
built from it. The Python `backend/` tree is retained only as the source
of the static `resources/` files (`mil_ranges.json`, `operators.json`)
and the legacy `adsb.proto`; deletion is a follow-up once production
cutover is verified.

The plan, agreed on with the project owner:

| Decision        | Choice                                                         |
|-----------------|----------------------------------------------------------------|
| Strategy        | Big-bang rewrite, single cutover                                |
| Wire compat     | Clean-slate API; frontend will be updated to match              |
| Database        | MongoDB stays; one-shot data migration script for schema cleanup |
| Live data       | `dump1090` (HTTP poll) + gRPC (streaming) only                  |
| Metadata        | All via `nighthawk-proxy` (single HTTP client)                  |
| Auth            | Clean-slate JWT in encrypted cookie + Argon2 hashing            |
| Users           | Reset on cutover (admin re-seeded from env, anonymous lazy)     |
| Observability   | OTEL-native (OTLP exporter); no direct Prometheus               |
| Rate limit      | `tower-governor` in-memory (Redis option deferred)              |
| Live state      | In-process snapshot via `ArcSwap` (no Redis on hot path)        |

## Architecture (Rust backend)

Clean Architecture with strict, link-time-enforced dependency direction:

```
       ┌─────────┐
       │ server  │  composition root, config, OTEL bootstrap
       └────┬────┘
            │
   ┌────────┴────────┐
   │       api       │  Axum routers, DTOs, mappers, SSE, middleware
   └────────┬────────┘
            │
   ┌────────┴────────┐
   │   application   │  use cases (FlightUpdater, AircraftCrawler, queries, auth)
   └────────┬────────┘
            │
   ┌────────┴────────┐         ┌──────────────────────────────┐
   │     domain      │ ◀────── │ adapters/{mongo,radar,        │
   │ entities, value │         │   metadata,auth}              │
   │ objects, ports, │         │ concrete impls of domain      │
   │ policies        │         │ ports                         │
   └─────────────────┘         └──────────────────────────────┘
```

- `domain` is dep-free (no I/O, no runtime, no framework). Owns entities,
  value objects, policies (modes classifier, callsign parser, circuit
  breaker), and ports (traits).
- `application` depends only on `domain`. Use cases hold their dependencies
  as `Arc<dyn Trait>`.
- `adapters/*` implement domain ports against concrete tech (MongoDB,
  dump1090 HTTP, gRPC, nighthawk-proxy, Argon2, JWT). One adapter crate per
  external boundary.
- `api` translates HTTP/SSE ↔ use cases. DTOs are separate types from domain
  entities — explicit `From<Flight> for FlightDto`-style mappers.
- `server` is the only place that knows the whole graph (composition root).

### Realtime pipeline

The hot path is streaming end-to-end. Both gRPC (genuinely streaming) and
dump1090 (poll, internally re-streamed) expose the same
`RadarSource::stream() -> Stream<PositionReport>` interface to the
application. The `FlightUpdater` consumes the stream continuously and
flushes on a periodic tick.

```
RadarSource impl  ──Stream<PositionReport>──▶  FlightUpdater.ingest()
                                               (fast, in-memory pending map)
                                                          │
                                                          ▼
                                            FlightUpdater.flush(now)
                                            ──── every flush_interval ────
                                              upserts flights
                                              persists positions (batched)
                                              prunes stale entries (TTL)
                                              publishes Delta to event bus
                                              swaps Arc<ArcSwap<LiveSnapshot>>
                                                          │
                                        ┌─────────────────┼─────────────────┐
                                        ▼                 ▼                 ▼
                                  Snapshot reads   broadcast::Sender   Persistence
                                  (GET /positions) (PositionEvent)     subscriber
                                                          │
                                                          ▼
                                                     SSE handlers
```

Key properties:
- The updater is the sole writer of the live snapshot — no locks on the
  read path; readers see immutable `Arc<LiveSnapshot>` snapshots.
- `ingest()` is hot: dedupes by ICAO24, sub-microsecond, no I/O.
- `flush()` is cold: runs on a configurable tick (default 2s), is the
  only place that touches Mongo on the hot path.
- Streaming sources push events as they arrive; the periodic flush
  decouples downstream cadence from upstream chatter.
- A `position_ttl_seconds` config drops aircraft that have not been seen
  for a while — required for streaming sources where "disappeared" is
  implicit.
- Live position deltas fan out via `tokio::sync::broadcast` — lossy by
  design (slow SSE clients get `Lagged`; the handler sends a fresh
  snapshot and resumes).

## Crate layout (`backend-rs/`)

```
backend-rs/
├── Cargo.toml                workspace + shared deps
└── crates/
    ├── domain/               pure entities, value objects, ports, policies
    ├── application/          use cases (FlightUpdater, Crawler, queries, auth)
    ├── adapters/
    │   ├── mongo/            FlightRepository, PositionRepository, … impls
    │   ├── radar/            dump1090 + gRPC RadarSource impls
    │   ├── metadata/         nighthawk-proxy MetadataSource impl
    │   └── auth/             Argon2 + JWT impls
    ├── api/                  Axum routers, DTOs, mappers, SSE, middleware
    └── server/               binary: main.rs, config, wiring, OTEL bootstrap
```

## Tech choices (Rust)

| Concern        | Crate                                                                          |
|----------------|--------------------------------------------------------------------------------|
| Runtime        | `tokio` (multi-thread)                                                         |
| HTTP server    | `axum` + `tower-http`                                                          |
| Rate limiting  | `tower-governor` (in-memory)                                                   |
| Database       | `mongodb` (official driver) + `bson`                                           |
| gRPC client    | `tonic` + `prost` (server lives in a separate repo)                            |
| HTTP client    | `reqwest` (rustls)                                                             |
| Auth           | `jsonwebtoken` + `argon2`; encrypted cookie via `axum-extra::cookie`           |
| Serde          | `serde`, `serde_json`                                                          |
| Tracing        | `tracing` + `tracing-subscriber` + `tracing-opentelemetry`                     |
| OTEL           | `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp` (gRPC default)      |
| Validation     | `validator`                                                                    |
| Errors         | `thiserror` in libs; `anyhow` at the binary edge                               |
| Mocks          | `mockall` (port mocks for use-case tests)                                      |
| Test infra     | `testcontainers` (Mongo), `wiremock` (HTTP)                                    |

## Conventions

### SOLID & DRY in practice

- **SRP**: one use case = one orchestration. Read-side and write-side are
  separate use cases, even when they touch the same entity.
- **OCP / DIP**: business code depends on traits in `domain::ports`, never
  on concrete clients. New radar sources / metadata sources are added by
  implementing the trait.
- **ISP**: ports are narrow (`FlightRepository`, `PositionRepository`,
  `AircraftRepository` — not one god-repo). The SSE handler depends on
  `PositionEventBus`, not on the radar source.
- **LSP**: trait impls are interchangeable; tests swap real adapters for
  `mockall`-generated mocks.
- **DRY**: one error type per layer with `From` conversions across boundaries;
  one DTO ↔ domain mapper per entity; one middleware stack assembled once.

### Code style

- `unsafe_code = "forbid"` workspace-wide.
- Clippy `pedantic` enabled; the few intentional opt-outs live in
  `Cargo.toml` (`workspace.lints.clippy`).
- No `unwrap()` / `expect()` in non-test code without a comment explaining
  the invariant.
- Value objects (`Icao24`, `Callsign`, `AirlineIcao`) enforce invariants at
  construction; downstream code never re-validates.
- Public functions and types should round-trip through `serde` where they
  cross the wire — newtypes implement `Deserialize` to validate on entry.

### Adding new code

1. New domain concept → add a value object or entity in `domain`.
2. New external dependency (DB, API, queue) → define a *narrow* trait in
   `domain::ports`, then implement it in `adapters/*`.
3. New endpoint → DTO + handler in `api`, use case in `application`. The
   handler should be thin — only translation.
4. Wire everything in `server::main` (composition root).

### Testing strategy

- `domain`: pure unit tests, no async, no I/O. Already covers value objects,
  ModeS classifier, callsign parser, circuit breaker.
- `application`: `mockall` mocks for ports → orchestration tests.
- `adapters/mongo`: `testcontainers` for real Mongo in CI.
- `adapters/radar`: `wiremock` for dump1090; in-process `tonic` server for
  gRPC.
- `api`: Axum router tests with stub use cases.
- `server`: one end-to-end smoke test bringing the whole graph up against
  test containers.

## Commands

### Rust (`backend-rs/`)

```bash
# Build everything
cargo build --workspace

# Lint (CI gate)
cargo clippy --workspace --all-targets -- -D warnings

# Test everything
cargo test --workspace

# Test a single crate
cargo test -p flightradar-domain

# Format check (CI gate)
cargo fmt --all --check
```

> Cargo auto-discovers the workspace from `backend-rs/Cargo.toml`. Either
> `cd backend-rs/` first, or use `cargo …` from inside the tree.

### Data migration (one-shot, after cutover)

```bash
# Renames legacy Python field names to the new Rust shape and drops
# the users collection (auth is re-seeded from ADMIN_EMAIL on boot).
MONGO_URI=mongodb://localhost:27017 \
MONGO_DB=flightradar \
    cargo run --bin flightradar-migrate --release
```

### Docker (production-equivalent image)

```bash
# Build
docker compose build

# Run
docker compose up -d

# Tail
docker compose logs -f flightradar
```

### Python backend (legacy, kept for reference data only)

The Python tree under `backend/` is no longer wired into the Docker
image; only its `resources/operators.json` and `resources/mil_ranges.json`
are copied in at build time. Run the legacy app if you want to compare
behaviour:

```bash
cd backend
uv sync
uv run uvicorn flightradar:app --reload --port 8083
```

### Frontend

```bash
cd frontend
npm install
npm run dev   # http://localhost:5173
```

## Configuration

The Rust backend will read environment variables only (no config files).
A complete list of env vars (legacy Python and new Rust) lives in
`.env.example`. Notable Rust-only additions during the migration:

- `OTEL_EXPORTER_OTLP_ENDPOINT` — OTLP collector endpoint.
- `OTEL_SERVICE_NAME` — defaults to `flightradar-backend`.
- `COOKIE_KEY` — 64-byte key for the encrypted JWT cookie (generate with
  `openssl rand -hex 64`).

## What NOT to do

- Do **not** import `mongodb`, `axum`, `tonic`, `reqwest`, or any I/O crate
  from `domain` or `application`. The `Cargo.toml` of each crate is the
  contract — if you need to add such a dep there, the design is wrong.
- Do **not** call `Clock::now()` from policies; pass `now` in.
- Do **not** put business rules in `adapters/*`. Adapters translate; they
  don't decide.
- Do **not** add wire-compatibility shims to the Python `backend/`. The
  cutover is big-bang; legacy compatibility is explicitly out of scope.
- Do **not** introduce a second domain god-trait. Keep ports narrow.
- Do **not** use `unwrap()` / `expect()` outside tests without a comment.
