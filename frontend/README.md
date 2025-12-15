# flightradar-ui

A Vue.js application for tracking and displaying flight radar information.

## Getting Started

### 1. Install dependencies
```bash
npm install
```

### 2. Configuration

Create a `.env` file in the frontend directory with the following variables:

```bash
# Flight API Configuration
VITE_FLIGHT_API_URL=http://localhost:8000/api/v1

# HERE Maps API Key (optional - get from https://developer.here.com/)
VITE_HERE_API_KEY=

# Flight API Authentication (optional)
VITE_FLIGHT_API_USER=
VITE_FLIGHT_API_PASSWORD=

# Mock Data Mode (set to 'true' to use mock data instead of real API)
VITE_MOCK_DATA=false
```

**Note**: A `.env` file should already exist with default values. Update `VITE_HERE_API_KEY` if you want map functionality.

### 3. Start development server
```bash
npm run dev
```

The frontend will be available at http://localhost:5173

### 4. Build for production
```bash
npm run build
```

### 5. Preview production build
```bash
npm run preview
```

### 6. Lint and fix files
```bash
npm run lint
```

## Mock Data Mode

If you don't have access to the backend API, you can set `VITE_MOCK_DATA=true` to use mock data instead. This allows you to develop and test the frontend without a running backend.

## Prerequisites

- **Node.js 18+** - Required for the application
- **Backend API** - The backend should be running at the URL specified in `VITE_FLIGHT_API_URL` (default: http://localhost:8000/api/v1)

## Error Handling

The application includes robust error handling for network errors and API issues. Check the browser console for any connection errors if the application isn't receiving data.
