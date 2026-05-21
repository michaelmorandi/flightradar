# Multi-stage build for production deployment.
#
# Build metadata can be passed as args (generate with: git describe --tags --always)
ARG BUILD_COMMIT=unknown
ARG BUILD_TIMESTAMP=unknown
ARG RUST_VERSION=1.82

# ---------------------------------------------------------------------------
# Stage 1: Frontend
# ---------------------------------------------------------------------------
FROM node:lts-alpine AS frontend-build
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm install
COPY frontend/ ./
# Placeholders substituted at container start by entrypoint.sh.
ENV VITE_FLIGHT_API_URL='${VITE_FLIGHT_API_URL}'
ENV VITE_HERE_API_KEY='${VITE_HERE_API_KEY}'
ENV VITE_MOCK_DATA='${VITE_MOCK_DATA}'
ENV VITE_ENABLE_INTERPOLATION='${VITE_ENABLE_INTERPOLATION}'
ENV VITE_UMAMI_ID='${VITE_UMAMI_ID}'
RUN npm run build

# ---------------------------------------------------------------------------
# Stage 2: Rust builder
# ---------------------------------------------------------------------------
FROM rust:${RUST_VERSION}-alpine AS rust-build
WORKDIR /work

# Toolchain prereqs: musl-dev for the linker, cmake/perl for ring,
# protoc-bin-vendored ships its own binary so no system protoc is needed.
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static \
        cmake make g++ perl

COPY backend-rs/ ./backend-rs/
WORKDIR /work/backend-rs
ARG BUILD_COMMIT
ARG BUILD_TIMESTAMP
ENV BUILD_COMMIT=${BUILD_COMMIT} \
    BUILD_TIMESTAMP=${BUILD_TIMESTAMP}
RUN cargo build --release --bin flightradar-server

# ---------------------------------------------------------------------------
# Stage 3: Runtime
# ---------------------------------------------------------------------------
FROM alpine:3 AS runtime
LABEL maintainer="Michael Morandi"

RUN apk add --no-cache tzdata nginx supervisor gettext ca-certificates libgcc \
    && ln -s /usr/share/zoneinfo/Europe/Zurich /etc/localtime

# Non-root user for the backend process.
RUN adduser -D -h /home/radar radar \
    && mkdir -p /var/log/supervisor /run/nginx /home/radar/resources \
    && chown -R radar:radar /var/log/supervisor /home/radar

# Backend binary
COPY --from=rust-build /work/backend-rs/target/release/flightradar-server \
        /usr/local/bin/flightradar-server

# Reference data (airline directory + military mode-S ranges) carried over
# from the legacy resources/ tree. These are static lookup tables; the
# Python code is gone but the data lives on.
COPY --chown=radar backend/resources/mil_ranges.json \
        backend/resources/operators.json \
        /home/radar/resources/

# Build metadata baked in for the /info endpoint.
ARG BUILD_COMMIT
ARG BUILD_TIMESTAMP
ENV BUILD_COMMIT=${BUILD_COMMIT} \
    BUILD_TIMESTAMP=${BUILD_TIMESTAMP} \
    AIRLINES_FILE=/home/radar/resources/operators.json

# Frontend assets
COPY --from=frontend-build /app/frontend/dist /usr/share/nginx/html

# nginx + supervisor + entrypoint
COPY contrib/nginx.conf /etc/nginx/http.d/default.conf
COPY contrib/supervisord.conf /etc/supervisord.conf
COPY contrib/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Ports: nginx on 80 (frontend + proxied API), backend on 8083 (direct).
EXPOSE 80 8083

ENTRYPOINT ["/entrypoint.sh"]
