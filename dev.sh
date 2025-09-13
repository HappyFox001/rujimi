#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_status "Starting Rujimi in development mode..."

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    print_error "Rust is not installed. Please install Rust from https://rustup.rs/"
    exit 1
fi

# Load environment variables from .env if it exists
if [ -f ".env" ]; then
    print_status "Loading environment variables from .env file"
    export $(cat .env | grep -v '^#' | xargs)
else
    print_warning "No .env file found. Using default configuration."
fi

# Set default development values
export PASSWORD="${PASSWORD:-123}"
export WEB_PASSWORD="${WEB_PASSWORD:-$PASSWORD}"
export PORT="${PORT:-7860}"
export FAKE_STREAMING="${FAKE_STREAMING:-true}"
export CONCURRENT_REQUESTS="${CONCURRENT_REQUESTS:-1}"
export CACHE_EXPIRY_TIME="${CACHE_EXPIRY_TIME:-21600}"
export MAX_CACHE_ENTRIES="${MAX_CACHE_ENTRIES:-500}"
export ENABLE_STORAGE="${ENABLE_STORAGE:-false}"
export RUST_LOG="${RUST_LOG:-rujimi=debug,tower_http=debug}"

print_status "Development configuration:"
print_status "  Port: $PORT"
print_status "  Log Level: $RUST_LOG"
print_status "  Storage: $ENABLE_STORAGE"

if [ -z "$GEMINI_API_KEYS" ]; then
    print_warning "GEMINI_API_KEYS environment variable is not set."
    print_warning "Please set it in your .env file for full functionality."
fi

print_success "Starting development server..."
print_status "Dashboard will be available at: http://localhost:$PORT"
print_status "API endpoint: http://localhost:$PORT/v1"
print_status "Press Ctrl+C to stop the development server"

echo ""

# Run in development mode with hot reload
cargo run