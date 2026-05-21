# Flightradar

Real-time ADS-B aircraft tracker. Vue 3 + TypeScript frontend, Rust
backend, MongoDB storage.

## Structure

- `frontend/` — Vue 3 + Vite single-page app, served by nginx in production.
- `backend-rs/` — Rust cargo workspace (9 crates) for the API + realtime
  pipeline.
- `resources/` — static reference data (`operators.json`, `mil_ranges.json`).
- `contrib/` — nginx, supervisord, container entrypoint.
- `docker-compose.yml`, `Dockerfile` — production container.

See [`AGENTS.md`](./AGENTS.md) for the full architecture, conventions,
and per-layer guides.

## Quick start

```bash
cp .env.example .env
# Edit .env — at minimum set MONGO_URI, MONGO_DB, RADAR_ENDPOINT, JWT_SECRET
docker compose up -d --build
```

Then:

- Frontend: <http://localhost:8080>
- Backend (direct): <http://localhost:8083/api/v1/info>

## Development

### Backend

```bash
cd backend-rs
cargo run --bin flightradar-server     # serves :8083
cargo test --workspace                 # full test suite (282 tests)
cargo clippy --workspace --all-targets -- -D warnings
```

The backend reads its config from environment variables only — see
`.env.example` for the full surface. Required: `MONGO_URI`, `MONGO_DB`,
`RADAR_ENDPOINT`, `JWT_SECRET`.

### Frontend

```bash
cd frontend
npm install
npm run dev                            # http://localhost:5173
```

The Vite dev server proxies API calls through `VITE_FLIGHT_API_URL`
(set in `.env`).

### Data migration (one-shot, before first Rust-backed deploy)

```bash
MONGO_URI=mongodb://... MONGO_DB=flightradar \
    cargo run --bin flightradar-migrate --release
```

Renames legacy Python field names (`modeS`→`icao24`, `icaoTypeCode`→
`type_code`, etc.) and drops the `users` collection so the admin can be
re-seeded from `ADMIN_EMAIL`/`ADMIN_PASSWORD` on next boot.

## Production topology

A single container runs three processes via supervisord:

- `nginx` on port 80 — serves the frontend static assets and reverse-proxies
  `/api/*` to the backend on `:8083`.
- `flightradar-server` (Rust) on port 8083 — JSON REST + SSE.
- (External) MongoDB and optional `nighthawk-proxy` for aircraft metadata.

CORS: `ALLOWED_ORIGINS` is required when the frontend lives on a
different origin than the backend (auth cookies don't accept wildcards).

OpenTelemetry: set `OTEL_EXPORTER_OTLP_ENDPOINT` to push traces; logs
are structured JSON on stdout regardless.

## License

See `LICENSE`.
