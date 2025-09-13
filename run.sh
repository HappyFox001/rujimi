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

# Check if binary exists
if [ ! -f "target/release/rujimi" ]; then
    print_error "Binary not found. Please run './build.sh' first."
    exit 1
fi

print_status "Starting Rujimi - High-performance Gemini API Proxy"

# Check for .env file
if [ -f ".env" ]; then
    print_status "Loading environment variables from .env file"
    set -a
    source .env
    set +a
else
    print_warning "No .env file found. Using default configuration."
    print_status "To configure the application, create a .env file with the following variables:"
    echo ""
    echo "# Basic configuration"
    echo "PASSWORD=your_password_here"
    echo "WEB_PASSWORD=your_web_password_here"
    echo "GEMINI_API_KEYS=your_api_key_1,your_api_key_2"
    echo ""
    echo "# Optional configuration"
    echo "FAKE_STREAMING=true"
    echo "CONCURRENT_REQUESTS=1"
    echo "CACHE_EXPIRY_TIME=21600"
    echo "MAX_CACHE_ENTRIES=500"
    echo "ENABLE_VERTEX=false"
    echo "SEARCH_MODE=false"
    echo ""
fi

# Check if GEMINI_API_KEYS is set
if [ -z "$GEMINI_API_KEYS" ]; then
    print_warning "GEMINI_API_KEYS environment variable is not set."
    print_warning "The application will start but API functionality may be limited."
    print_status "Please set GEMINI_API_KEYS in your .env file or environment."
fi

# Set default values if not provided
export PASSWORD="${PASSWORD:-123}"
export WEB_PASSWORD="${WEB_PASSWORD:-$PASSWORD}"
export PORT="${PORT:-7860}"
export FAKE_STREAMING="${FAKE_STREAMING:-true}"
export CONCURRENT_REQUESTS="${CONCURRENT_REQUESTS:-1}"
export CACHE_EXPIRY_TIME="${CACHE_EXPIRY_TIME:-21600}"
export MAX_CACHE_ENTRIES="${MAX_CACHE_ENTRIES:-500}"
export ENABLE_STORAGE="${ENABLE_STORAGE:-true}"
export STORAGE_DIR="${STORAGE_DIR:-./rujimi_data}"

# Create storage directory if it doesn't exist
if [ "$ENABLE_STORAGE" = "true" ]; then
    mkdir -p "$STORAGE_DIR"
    print_status "Storage directory: $STORAGE_DIR"
fi

print_status "Configuration:"
print_status "  Port: $PORT"
print_status "  Password: $(echo $PASSWORD | sed 's/./*/g')"
print_status "  Fake Streaming: $FAKE_STREAMING"
print_status "  Concurrent Requests: $CONCURRENT_REQUESTS"
print_status "  Cache Entries: $MAX_CACHE_ENTRIES"
print_status "  Storage: $ENABLE_STORAGE"

print_success "Starting application..."
print_status "Dashboard will be available at: http://localhost:$PORT"
print_status "API endpoint: http://localhost:$PORT/v1"
print_status "Press Ctrl+C to stop the application"

echo ""

# Run the application
exec ./target/release/rujimi