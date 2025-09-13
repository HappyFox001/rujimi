#!/bin/bash

set -e

echo "🚀 Building Rujimi - High-performance Gemini API Proxy"

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

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    print_error "Rust is not installed. Please install Rust from https://rustup.rs/"
    exit 1
fi

# Check if Node.js is installed
if ! command -v node &> /dev/null; then
    print_error "Node.js is not installed. Please install Node.js from https://nodejs.org/"
    exit 1
fi

print_status "Building frontend applications..."

# Build the page frontend
if [ -d "page" ]; then
    print_status "Building page frontend..."
    cd page
    if [ ! -d "node_modules" ]; then
        print_status "Installing page dependencies..."
        npm install
    fi
    npm run build
    cd ..
    print_success "Page frontend built successfully"
else
    print_warning "Page directory not found, skipping page build"
fi

# Build the hajimiUI frontend
if [ -d "hajimiUI" ]; then
    print_status "Building hajimiUI frontend..."
    cd hajimiUI
    if [ ! -d "node_modules" ]; then
        print_status "Installing hajimiUI dependencies..."
        npm install
    fi
    npm run build
    cd ..
    print_success "hajimiUI frontend built successfully"
else
    print_warning "hajimiUI directory not found, skipping hajimiUI build"
fi

print_status "Building Rust backend..."

# Build the Rust backend
cargo build --release

print_success "Rust backend built successfully"

# Copy templates if they don't exist in the binary location
if [ ! -d "target/release/templates" ]; then
    print_status "Copying templates..."
    cp -r src/templates target/release/
fi

# Copy frontend dist directories
if [ -d "page/dist" ]; then
    print_status "Copying page frontend..."
    cp -r page/dist target/release/page/
fi

if [ -d "hajimiUI/dist" ]; then
    print_status "Copying hajimiUI frontend..."
    cp -r hajimiUI/dist target/release/hajimiUI/
fi

print_success "Build completed successfully!"
print_status "Binary location: target/release/rujimi"
print_status "To run the application:"
print_status "  ./target/release/rujimi"
print_status "or use the run script:"
print_status "  ./run.sh"