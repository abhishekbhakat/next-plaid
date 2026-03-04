#!/bin/bash
# Setup script for remote GPU server (run this on your 4070 Super machine)
# This clones from git@github.com:abhishekbhakat/next-plaid.git

set -e

echo "=========================================="
echo "NextPlaid Remote GPU Server Setup"
echo "From: git@github.com:abhishekbhakat/next-plaid.git"
echo "=========================================="

# Check NVIDIA GPU
if ! command -v nvidia-smi &> /dev/null; then
    echo "ERROR: nvidia-smi not found. Install NVIDIA drivers first."
    exit 1
fi

echo ""
echo "GPU Info:"
nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader

# Check Docker
if ! command -v docker &> /dev/null; then
    echo "ERROR: Docker not found. Install Docker first."
    exit 1
fi

# Check NVIDIA Container Toolkit
if ! docker info | grep -q "nvidia"; then
    echo ""
    echo "WARNING: NVIDIA Container Toolkit may not be installed."
    echo "Install it for GPU support:"
    echo "  https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/install-guide.html"
    echo ""
    read -p "Continue anyway? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Clone or update repository from your fork
echo ""
echo "Cloning from git@github.com:abhishekbhakat/next-plaid.git..."
cd ~

if [ -d "next-plaid" ]; then
    echo "Repository already exists. Pulling latest changes..."
    cd next-plaid
    git pull origin main || echo "Note: Could not pull updates. Continuing with local version."
else
    echo "Cloning repository..."
    git clone git@github.com:abhishekbhakat/next-plaid.git
    cd next-plaid
fi

# Create server directory structure
echo ""
echo "Creating server directories..."
mkdir -p ~/nextplaid-server/data/indices
mkdir -p ~/nextplaid-server/models

# Copy docker-compose.yml from your fork
cd ~/nextplaid-server
if [ -f "~/next-plaid/remote-gpu-wrapper/docker-compose.yml" ]; then
    cp ~/next-plaid/remote-gpu-wrapper/docker-compose.yml .
    echo "Copied docker-compose.yml from your fork"
else
    echo "Creating docker-compose.yml from embedded template..."
    cat > docker-compose.yml << 'EOF'
services:
  nextplaid-gpu:
    image: ghcr.io/lightonai/next-plaid:cuda-1.0.8
    container_name: nextplaid-gpu
    ports:
      - "8080:8080"
    volumes:
      - ./data/indices:/data/indices
      - ~/.cache/huggingface:/models
    environment:
      - RUST_LOG=info
      - NVIDIA_VISIBLE_DEVICES=all
    command:
      - --host
      - "0.0.0.0"
      - --port
      - "8080"
      - --index-dir
      - /data/indices
      - --model
      - lightonai/LateOn-Code
      - --cuda
      - --batch-size
      - "64"
      - --parallel
      - "4"
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]
    restart: unless-stopped
EOF
fi

# Start server
echo ""
echo "Starting NextPlaid GPU server..."
docker-compose up -d

echo ""
echo "Waiting for server to start..."
sleep 10

# Health check
echo ""
echo "Checking server health..."
if curl -s http://localhost:8080/health > /dev/null; then
    echo "✓ Server is running!"
    curl -s http://localhost:8080/health | head -20
else
    echo "✗ Server not responding. Check logs: docker-compose logs -f"
    exit 1
fi

echo ""
echo "=========================================="
echo "Setup complete!"
echo "=========================================="
echo ""
echo "Server is running on: http://$(hostname -I | awk '{print $1}'):8080"
echo ""
echo "Useful commands:"
echo "  View logs:    docker-compose logs -f"
echo "  Stop server:  docker-compose down"
echo "  Restart:      docker-compose restart"
echo ""
echo "From your laptop, connect via:"
echo "  1. SSH tunnel: ssh -L 8080:localhost:8080 user@this-server-ip"
echo "  2. Direct:     http://this-server-ip:8080 (if firewall allows)"
echo ""
echo "Repository location: ~/next-plaid"
echo "Data location:       ~/nextplaid-server"
echo ""
