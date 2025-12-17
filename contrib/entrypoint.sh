#!/bin/sh

echo "Starting Flightradar unified container..."

# Replace env vars in JavaScript files at runtime (for frontend)
echo "Replacing env vars in JS files..."

# Set defaults if not provided
export VITE_FLIGHT_API_URL=${VITE_FLIGHT_API_URL:-http://localhost:8083/api/v1}
export VITE_HERE_API_KEY=${VITE_HERE_API_KEY:-}
export VITE_FLIGHT_API_USER=${VITE_FLIGHT_API_USER:-}
export VITE_FLIGHT_API_PASSWORD=${VITE_FLIGHT_API_PASSWORD:-}
export VITE_MOCK_DATA=${VITE_MOCK_DATA:-false}
export VITE_ENABLE_INTERPOLATION=${VITE_ENABLE_INTERPOLATION:-true}

echo "Using VITE_FLIGHT_API_URL: $VITE_FLIGHT_API_URL"

for file in /usr/share/nginx/html/assets/index-*.js /usr/share/nginx/html/assets/FlightLog-*.js;
do
  if [ -f "$file" ]; then
    echo "Processing $file ...";

    # Use the existing JS file as template (only create template once)
    if [ ! -f "$file.tmpl" ]; then
      cp "$file" "$file.tmpl"
    fi

    # Replace placeholders with actual environment variables
    envsubst '${VITE_FLIGHT_API_URL} ${VITE_HERE_API_KEY} ${VITE_FLIGHT_API_USER} ${VITE_FLIGHT_API_PASSWORD} ${VITE_MOCK_DATA} ${VITE_ENABLE_INTERPOLATION}' < "$file.tmpl" > "$file"
  fi
done

echo "Starting services via supervisor..."
exec /usr/bin/supervisord -c /etc/supervisord.conf
