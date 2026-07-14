#!/bin/bash
set -e

cd "$(dirname "$0")"

# Set HDF5 path for macOS with Homebrew
export HDF5_DIR=/opt/homebrew/opt/hdf5@1.10

# Load .env file if it exists
if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
elif [ -f ../backend/.env ]; then
    export $(grep -v '^#' ../backend/.env | xargs)
elif [ -f ../.env ]; then
    export $(grep -v '^#' ../.env | xargs)
else
    echo "Warning: No .env file found"
fi

# Check for API key
if [ -z "$METEO_FRANCE_API_KEY" ]; then
    echo "Error: METEO_FRANCE_API_KEY not set"
    echo "Create a .env file with: METEO_FRANCE_API_KEY=your_key"
    exit 1
fi

# Build if needed
if [ ! -f target/release/rain-radar ] || [ Cargo.toml -nt target/release/rain-radar ]; then
    echo "Building..."
    cargo build --release
fi

# Run the server
echo "Starting rain-radar server on http://localhost:8080"
./target/release/rain-radar
