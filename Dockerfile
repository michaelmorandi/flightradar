# Multi-stage build for production deployment
# Stage 1: Build frontend
FROM node:lts-alpine AS frontend-build
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm install
COPY frontend/ ./
# Set VITE environment variables as placeholders for runtime substitution
# These will be replaced by envsubst at container startup
ENV VITE_FLIGHT_API_URL='${VITE_FLIGHT_API_URL}'
ENV VITE_HERE_API_KEY='${VITE_HERE_API_KEY}'
ENV VITE_FLIGHT_API_USER='${VITE_FLIGHT_API_USER}'
ENV VITE_FLIGHT_API_PASSWORD='${VITE_FLIGHT_API_PASSWORD}'
ENV VITE_MOCK_DATA='${VITE_MOCK_DATA}'
ENV VITE_ENABLE_INTERPOLATION='${VITE_ENABLE_INTERPOLATION}'
RUN npm run build

# Stage 2: Production image
FROM python:3-alpine

LABEL maintainer="Michael Morandi"

# Install system prerequisites
RUN apk update && \
    apk upgrade && \
    apk add --update tzdata mongodb-tools nginx supervisor gettext && \
    ln -s /usr/share/zoneinfo/Europe/Zurich /etc/localtime

# Install uv
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /bin/

# Create application user
RUN adduser -D radar && \
    mkdir -p /var/log/supervisor && \
    mkdir -p /run/nginx && \
    chown -R radar:radar /var/log/supervisor

USER radar
WORKDIR /home/radar

# Install Python dependencies with uv
COPY --chown=radar backend/pyproject.toml backend/uv.lock ./
RUN uv sync --frozen

# Install backend application
COPY --chown=radar backend/app app
RUN mkdir -p resources
COPY --chown=radar backend/resources/mil_ranges.csv backend/resources/meta.json resources/
COPY --chown=radar backend/flightradar.py ./

# Copy frontend build from previous stage
USER root
COPY --from=frontend-build /app/frontend/dist /usr/share/nginx/html

# Copy Nginx configuration
COPY contrib/nginx.conf /etc/nginx/http.d/default.conf

# Copy supervisor configuration
COPY contrib/supervisord.conf /etc/supervisord.conf

# Copy startup script
COPY contrib/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Expose HTTP port (nginx) and backend port (uvicorn)
EXPOSE 80 8083

# Start both services via supervisor
ENTRYPOINT ["/entrypoint.sh"]
