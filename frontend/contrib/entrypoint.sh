#!/bin/sh

# Replace env vars in JavaScript files at runtime
echo "Replacing env vars in JS files..."

# Set defaults if not provided
export VITE_FLIGHT_API_URL=${VITE_FLIGHT_API_URL:-}
export VITE_HERE_API_KEY=${VITE_HERE_API_KEY:-}
export VITE_FLIGHT_API_USER=${VITE_FLIGHT_API_USER:-}
export VITE_FLIGHT_API_PASSWORD=${VITE_FLIGHT_API_PASSWORD:-}
export VITE_MOCK_DATA=${VITE_MOCK_DATA:-false}

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
    envsubst '${VITE_FLIGHT_API_URL} ${VITE_HERE_API_KEY} ${VITE_FLIGHT_API_USER} ${VITE_FLIGHT_API_PASSWORD} ${VITE_MOCK_DATA}' < "$file.tmpl" > "$file"
  fi
done

echo "Starting Nginx..."
nginx -g 'daemon off;'