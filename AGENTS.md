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

Big-bang Python → Rust migration is in progress on the
`claude/plan-rust-migration-T0Zet` branch. Until the Rust backend reaches
parity, `backend/` is the authoritative implementation. Both trees compile
side by side; nothing in `backend-rs/` is wired into the Docker image yet.

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

The hot path is intentionally lock-free:

```
RadarSource impl  ──Stream<PositionReport>──▶  FlightUpdater (single task)
                                                       │
                                          publishes new immutable snapshot
                                                       │
                                              Arc<ArcSwap<FlightState>>
                                                       │
                                        ┌──────────────┼──────────────┐
                                        ▼              ▼              ▼
                                  Snapshot reads   broadcast::Sender   …
                                  (GET /positions) (PositionEvent)
                                                       │
                                                 ┌─────┴─────┐
                                                 ▼           ▼
                                            SSE handler   Persistence subscriber
                                            (per-client)  (batched Mongo writes)
```

- One updater task is the sole writer of `FlightState`. No locks.
- Readers (HTTP and SSE init) hit `ArcSwap` — sub-microsecond.
- Live position deltas fan out via `tokio::sync::broadcast` — lossy by
  design (slow consumers get `Lagged`; SSE handler sends a fresh snapshot
  and resumes).
- Mongo writes are a separate subscriber — never on the ingestion hot path.

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

### Python backend (`backend/`)

```bash
cd backend
uv sync
uv run uvicorn flightradar:app --reload --port 8083
uv run pytest
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
