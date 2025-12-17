# Flightradar Monorepo

This monorepo contains both the frontend and backend applications for the Flightradar system.

## Structure

- `backend/` - Python FastAPI application
- `frontend/` - Vue.js TypeScript application
- `contrib/` - Configuration files for deployment

## Deployment (Production)

The application is containerized as a single Docker image that serves both the frontend and backend.

### Using Docker Compose (Recommended)

```bash
# Create .env file from example (first time only)
cp .env.example .env
# Edit .env and add your configuration

# Build and start the application
docker-compose up -d

# View logs
docker-compose logs -f

# Stop the application
docker-compose down
```

**Note:** Docker Compose reads environment variables from `.env` file in the project root (same directory as `docker-compose.yml`). Copy your `backend/.env` to the root or create a new one based on `.env.example`.

The application will be available at:
- Frontend: [http://localhost:8080](http://localhost:8080)
- Backend API: [http://localhost:8083/api/v1/docs](http://localhost:8083/api/v1/docs)

### Using Docker directly

```bash
# Build the image
docker build -t flightradar .

# Run the container
docker run -p 80:80 \
  -e VITE_FLIGHT_API_URL=/api \
  -e VITE_HERE_API_KEY=your_key_here \
  flightradar
```

### Environment Variables

Configure the application using environment variables:

**Frontend:**
- `VITE_FLIGHT_API_URL` - Full API URL with protocol, host, and path (e.g., `http://localhost:8083/api/v1` for local Docker, or `https://your-domain.com/api/v1` for production)
- `VITE_HERE_API_KEY` - HERE Maps API key
- `VITE_FLIGHT_API_USER` - API authentication user
- `VITE_FLIGHT_API_PASSWORD` - API authentication password
- `VITE_MOCK_DATA` - Use mock data (default: `false`)
- `VITE_ENABLE_INTERPOLATION` - Enable aircraft position interpolation (default: `true`)

**Backend:**
Add your backend environment variables as needed (MongoDB URI, etc.)

## Development

For development, run the frontend and backend separately using their respective dev servers:

### Backend Development
```bash
cd backend
uv sync
uv run uvicorn flightradar:app --reload --port 8083
```

### Frontend Development
```bash
cd frontend
npm install
npm run dev
```

The frontend dev server (Vite) will run on port 5173 and can proxy API requests to the backend on port 8083.

## Architecture

**Production Setup:**
- Nginx serves static frontend files on port 80
- Backend FastAPI runs via uvicorn on port 8083 (exposed)
- Both services run in a single container managed by supervisord
- Frontend is built during the Docker image build process with the API URL baked in
- Frontend accesses backend directly via FQDN (e.g., `http://localhost:8083/api/v1` or `https://api.your-domain.com/api/v1`)

**Development Setup:**
- Frontend: Vite dev server with hot reload
- Backend: uvicorn with --reload flag
- Separate processes for better development experience
