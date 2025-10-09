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

## Quick Start

### 1. Install Dependencies
```bash
cd flightradar-backend
uv sync
```

### 2. Configure Application

#### Option A: Config File (Recommended)
```bash
# Copy sample config
cp contrib/samples/config.json config.json

# Edit config.json with your settings
```

#### Option B: Environment Variables
```bash
export MONGODB_URI="mongodb://localhost:27017/"
export MONGODB_DB_NAME="flightradar"
export SERVICE_URL="http://your-radar-server:8080"
export SERVICE_TYPE="vrs"  # or "dump1090"
```

### 3. Initialize Database
```bash
uv run python flightradar.py initschema
```

### 4. Start Development Server
```bash
# Standard development server
uv run python -m uvicorn flightradar:app --host 0.0.0.0 --port 8000 --reload

# Alternative command
uv run uvicorn flightradar:app --host 0.0.0.0 --port 8000 --reload
```

### 5. Test SSE Endpoints
```bash
# Test live positions stream
curl -N -H "Accept: text/event-stream" http://localhost:8000/api/v1/sse/positions/live

# Test flight-specific stream
curl -N -H "Accept: text/event-stream" http://localhost:8000/api/v1/sse/flights/ABC123/positions
```

## Configuration Options

### Core Settings
| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `serviceUrl` | yes | - | URL to your radar service |
| `type` | no | `vrs` | Service type: `vrs` or `dump1090` |
| `dataFolder` | no | `resources` | Path to resources folder |
| `militaryOnly` | no | `false` | Filter non-military aircraft |

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
docker build -t flightradar-backend .
docker run -d -p 8000:8000 --env-file .env flightradar-backend
```

## 🔌 API Endpoints

### REST API
- `GET /api/v1/flights` - List all flights
- `GET /api/v1/flights/{id}` - Get specific flight
- `GET /api/v1/flights/{id}/positions` - Get flight position history
- `GET /api/v1/positions` - Get all current positions
- `GET /api/v1/info` - Application metadata
- `GET /api/v1/alive` - Health check
- `GET /api/v1/ready` - Readiness check

### Server-Sent Events (SSE)
- `GET /api/v1/sse/positions/live` - Real-time position updates
- `GET /api/v1/sse/flights/{flight_id}/positions` - Flight-specific updates

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