# FlightRadar Backend

A FastAPI-based backend for tracking and managing real-time ADS-B flight data with Server-Sent Events (SSE) streaming capabilities.

## Features

- **Real-time Flight Tracking**: Live ADS-B data collection from radar servers
- **Server-Sent Events (SSE)**: Real-time position streaming to web clients
- **Interactive REST API**: Comprehensive flight and position data endpoints
- **Aircraft Data Enrichment**: Optional web crawling for unknown aircraft
- **Military Flight Filtering**: Configurable military-only tracking mode

## Prerequisites

1. **Python 3.11+** - Required for the application
2. **UV Package Manager** - [Install UV](https://docs.astral.sh/uv/getting-started/installation/)
3. **MongoDB** - Database for flight and position data
4. **ADS-B Data Source** - dump1090 or Virtual Radar Server
5. **MongoDB Integration** A MongoDB server to store flight data

## Getting Started

### 1. Install Dependencies
```bash
cd backend
uv sync
```

### 2. Configure Application

Set the required environment variables:

```bash
export MONGODB_URI="mongodb://localhost:27017/"
export MONGODB_DB_NAME="flightradar"
export SERVICE_URL="http://your-radar-server:8080"
export SERVICE_TYPE="dump1090"  # or "vrs"
```

Or create a `.env` file in the backend directory:

```bash
MONGODB_URI=mongodb://localhost:27017/
MONGODB_DB_NAME=flightradar
SERVICE_URL=http://your-radar-server:8080
SERVICE_TYPE=dump1090
```

### 3. Initialize Database
```bash
uv run python flightradar.py initschema
```

### 4. Start Development Server
```bash
uv run uvicorn flightradar:app --host 0.0.0.0 --port 8000 --reload
```

The backend API will be available at http://localhost:8000

- **API Documentation (Swagger)**: http://localhost:8000/docs
- **API Documentation (ReDoc)**: http://localhost:8000/redoc

### 5. Test SSE Endpoints
```bash
# Test live flight data stream (positions, categories, callsigns)
curl -N -H "Accept: text/event-stream" http://localhost:8000/api/v1/live/stream

# Test flight-specific stream
curl -N -H "Accept: text/event-stream" http://localhost:8000/api/v1/flights/ABC123/positions/stream
```

## Configuration Options

Note: This application reads configuration only from environment variables or a `.env` file. It does not support `config.json` or other file-based configuration formats.

### Core Settings
| Option (env var) | Required | Default | Description |
|--------|----------|---------|-------------|
| `SERVICE_URL` | yes | - | URL to your radar service |
| `SERVICE_TYPE` | no | `vrs` | Service type: `vrs` or `dump1090` |
| `DATA_FOLDER` | no | `resources` | Path to resources folder |
| `MIL_ONLY` | no | `false` | Filter non-military aircraft |

### Database Configuration
| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `mongodb_uri` | yes | - | MongoDB connection string |
| `mongodb_db_name` | yes | - | MongoDB database name |
| `deleteAfterMinutes` | no | `1440` | Data retention period (0 = indefinite) |

### Advanced Options
| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `crawlUnknownAircraft` | no | `false` | Web lookup for unknown aircraft |
| `googleMapsApiKey` | no | - | For map rendering |

## Production Deployment

### Option 1: Uvicorn
```bash
uv run uvicorn flightradar:app --host 0.0.0.0 --port 8000 --workers 4
```

### Option 2: Docker (recommended)
```bash
# Build with automatic git version capture
./contrib/build.sh

# Or manually (generates meta.json first, then builds):
./contrib/generate-meta.sh
docker build -t flightradar:latest .

# Run the container
docker run -d -p 8000:8000 --env-file .env flightradar:latest
```

The Docker image automatically captures the git tag (or commit hash if no tag exists) at build time. This version information is:
- Logged on server startup
- Accessible via `/api/v1/info` endpoint in the `commit_id` field

## 🔌 API Endpoints

### REST API
- `GET /api/v1/flights` - List all flights
- `GET /api/v1/flights/{id}` - Get specific flight
- `GET /api/v1/flights/{id}/positions` - Get flight position history
- `GET /api/v1/positions` - Get all current positions
- `GET /api/v1/info` - Application metadata (commit_id, build_timestamp)
- `GET /api/v1/alive` - Health check
- `GET /api/v1/ready` - Readiness check

### Server-Sent Events (SSE)
- `GET /api/v1/live/stream` - Real-time flight data stream (positions, categories, callsigns)
- `GET /api/v1/flights/{flight_id}/positions/stream` - Flight-specific position updates

### Interactive API Documentation
- **Swagger UI**: http://localhost:8000/docs
- **ReDoc**: http://localhost:8000/redoc

## Development & Testing

### Run Tests
```bash
# Run all tests
uv run pytest

# Run with coverage
uv run pytest --cov=app tests/
```
### Health Checks
- `/api/v1/alive` - Basic health check
- `/api/v1/ready` - Readiness with scheduler status

### Environment Variables for Production
```bash
export MONGODB_URI="mongodb://user:pass@localhost:27017/"
export SERVICE_URL="https://your-radar-server.com"
export SECRET_KEY="your-secret-key"
export ENVIRONMENT="production"
```