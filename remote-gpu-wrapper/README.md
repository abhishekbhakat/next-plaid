# Remote GPU + NextPlaid Wrapper

Self-hosted semantic code search with a remote GPU server (e.g., NVIDIA 4070 Super).

> **Repository:** `git@github.com:abhishekbhakat/next-plaid.git`

## Architecture

```
Your Laptop                    Remote GPU Server (4070 Super)
┌─────────────────┐           ┌─────────────────────────────┐
│  Tree-sitter    │           │  Docker + NextPlaid API     │
│  (parse code)   │ ────────► │  - ONNX Runtime (CUDA)      │
│                 │  HTTP API │  - LateOn-Code model        │
│  NextPlaid      │ ◄──────── │  - GPU inference            │
│  Client         │ embeddings│  - Index storage            │
│                 │           │                             │
│  (Lightweight)  │           │  (Heavy compute)            │
└─────────────────┘           └─────────────────────────────┘
```

## How It Works

1. **Your laptop**: Parses code with Tree-sitter, sends text to remote server
2. **Remote server**: Encodes with GPU-accelerated ONNX Runtime
3. **Remote server**: Stores index, performs MaxSim search
4. **Your laptop**: Receives search results

## Prerequisites

### Remote Server (GPU machine)

- NVIDIA GPU (4070 Super, etc.)
- Docker + NVIDIA Container Toolkit (Linux) or NVIDIA Docker (Windows)
- Public IP or SSH tunnel

### Your Laptop

- Python 3.9+
- Tree-sitter parsers
- Windows 11, macOS, or Linux

## Setup

### 1. Remote Server Setup

#### Linux/macOS (GPU Server)

On your GPU machine (4070 Super), run the automated setup:

```bash
# Download and run setup script from your fork
curl -sSL https://raw.githubusercontent.com/abhishekbhakat/next-plaid/main/remote-gpu-wrapper/setup-remote-server.sh | bash
```

Or manually with Docker:

```bash
# Run NextPlaid API with CUDA support
docker run --gpus all -d \
  --name nextplaid-gpu \
  -p 8080:8080 \
  -v ~/nextplaid-data:/data/indices \
  -v ~/.cache/huggingface:/models \
  -e RUST_LOG=info \
  ghcr.io/lightonai/next-plaid:cuda-1.0.8 \
  --host 0.0.0.0 \
  --port 8080 \
  --index-dir /data/indices \
  --model lightonai/LateOn-Code \
  --cuda \
  --batch-size 64 \
  --parallel 4
```

Verify it's running:

```bash
curl http://localhost:8080/health
```

#### Windows 11 (GPU Server)

**Note for Windows 11:** If you encounter `CUDA driver version is insufficient` errors, you need to build a custom image with CUDA 12.6 (see [Building Custom CUDA 12.6 Image](#building-custom-cuda-126-image) below).

For standard setup with pre-built image:

```powershell
# Run NextPlaid API with CUDA support on Windows
docker run --gpus all -d `
  --name nextplaid-gpu `
  -p 8080:8080 `
  -v ${env:USERPROFILE}\nextplaid-data:/data/indices `
  -v ${env:USERPROFILE}\.cache\huggingface:/models `
  -e RUST_LOG=info `
  ghcr.io/lightonai/next-plaid:cuda-1.0.8 `
  --host 0.0.0.0 `
  --port 8080 `
  --index-dir /data/indices `
  --model lightonai/LateOn-Code `
  --cuda `
  --batch-size 64 `
  --parallel 4
```

Or use Docker Compose on Windows:

```powershell
# Standard CUDA 12.4 (may not work on newer Windows drivers)
docker-compose up -d

# For CUDA 12.6 (recommended for Windows 11 with driver 551+)
docker-compose -f docker-compose.yml -f docker-compose.cuda126.yml up -d
```

Verify it's running (PowerShell):

```powershell
Invoke-RestMethod -Uri http://localhost:8080/health | ConvertTo-Json
```

### 2. Network Access

#### Option A: Direct access (if server has public IP)

```
Your laptop → http://server-ip:8080
```

#### Option B: SSH tunnel (recommended for security)

**Linux/macOS:**
```bash
# On your laptop
ssh -L 8080:localhost:8080 user@gpu-server-ip

# Then connect to localhost:8080
```

**Windows 11 (PowerShell):**
```powershell
# On your laptop
ssh -L 8080:localhost:8080 user@gpu-server-ip

# Or use Windows Terminal / PuTTY with port forwarding
# Then connect to localhost:8080
```

**Windows 11 (PuTTY):**
1. Open PuTTY → Connection → SSH → Tunnels
2. Source port: `8080`
3. Destination: `localhost:8080`
4. Click "Add", then connect to your server

#### Option C: VPN/WireGuard

```
Both machines on same private network
```

### 3. Laptop Setup

#### Linux/macOS

```bash
# Clone your fork
git clone git@github.com:abhishekbhakat/next-plaid.git
cd next-plaid/remote-gpu-wrapper

# Install dependencies
pip install -r requirements.txt

# Copy and edit config
cp example-env .env
# Edit .env with your server details
```

#### Windows 11

```powershell
# Clone your fork
git clone git@github.com:abhishekbhakat/next-plaid.git
cd next-plaid\remote-gpu-wrapper

# Install dependencies
pip install -r requirements.txt

# Copy and edit config
copy example-env .env
# Edit .env with your server details (use Notepad, VS Code, etc.)
notepad .env
```

**Note for Windows users:**
- Use PowerShell or Windows Terminal for best compatibility
- Python 3.9+ required (install from Microsoft Store or python.org)
- Git for Windows recommended: https://git-scm.com/download/win

## Usage

### Index a Project

```python
from remote_gpu_wrapper import RemoteGPUCodeSearch

search = RemoteGPUCodeSearch(
    server_url="http://gpu-server:8080",  # or localhost:8080 with SSH tunnel
    index_name="my_project"
)

# Index a codebase
search.index_project("/path/to/code")
```

### Search

```python
# Search
results = search.search("database connection pooling", k=10)

for r in results:
    print(f"{r['file']}:{r['line']} - Score: {r['score']:.3f}")
    print(f"  {r['name']} ({r['unit_type']})")
```

### Interactive Mode

**Linux/macOS:**
```bash
python example.py --interactive --server http://gpu-server:8080
```

**Windows 11:**
```powershell
python example.py --interactive --server http://gpu-server:8080
```

## Comparison with Baseten Wrapper

| Aspect | Baseten Wrapper | Remote GPU Wrapper |
|--------|-----------------|-------------------|
| Hosting | Baseten cloud | Your own hardware |
| Cost | Per-use pricing | Free (after hardware) |
| Setup complexity | Low | Medium |
| Code privacy | Sent to Baseten | Stays on your machines |
| GPU required | No | Yes (on server) |
| Network | Internet | Local network or VPN |
| Index location | Local laptop | Remote server |

## Security Considerations

Since this is self-hosted:

1. **Use SSH tunnel or VPN** - Don't expose port 8080 to internet
2. **Firewall rules** - Restrict access to your laptop's IP
3. **No authentication** - NextPlaid API has no built-in auth

Example with nginx reverse proxy + basic auth:

```nginx
server {
    listen 80;
    location / {
        auth_basic "NextPlaid";
        auth_basic_user_file /etc/nginx/.htpasswd;
        proxy_pass http://localhost:8080;
    }
}
```

## Troubleshooting

### Connection refused

- Check SSH tunnel is active
- Verify firewall allows port 8080
- Check Docker container is running:
  - Linux/macOS: `docker ps`
  - Windows: `docker ps` or Docker Desktop GUI

### CUDA out of memory

- Reduce `--batch-size` (try 32 or 16)
- Reduce `--parallel` (try 2)
- Use smaller model: `lightonai/LateOn-Code-edge`

### Slow encoding

- Check GPU is being used:
  - Linux/macOS: `nvidia-smi`
  - Windows: `nvidia-smi` or Task Manager → Performance → GPU
- Increase `--parallel` for more concurrency
- Use `--int8` for faster inference

### Windows-specific issues

**Docker Desktop not starting:**
- Ensure WSL2 is installed: `wsl --install` in PowerShell as Admin
- Enable virtualization in BIOS

**GPU not detected in Docker:**
- Install NVIDIA Container Toolkit for Windows
- Ensure Docker Desktop → Settings → Resources → WSL Integration is enabled

**SSH tunnel issues:**
- Use Windows Terminal or PowerShell (not CMD)
- Or use PuTTY with port forwarding configuration

**Path issues:**
- Use forward slashes `/` or escaped backslashes `\\` in Python paths
- Or use raw strings: `r"C:\Users\name\project"`

**CUDA driver version error on Windows:**
If you see: `CUDA failure 35: CUDA driver version is insufficient for CUDA runtime version`

This means your Windows NVIDIA driver is newer than what the pre-built image supports. You have two options:

1. **Build custom CUDA 12.6 image** (recommended):
   ```powershell
   # Clone the repository
git clone git@github.com:abhishekbhakat/next-plaid.git
   cd next-plaid

   # Build the CUDA 12.6 image
   docker build -t next-plaid-api:cuda-12.6 `
     -f next-plaid-api/Dockerfile.cuda126 `
     --target runtime-cuda .

   # Run with the custom image
   docker run --gpus all -d `
     --name nextplaid-gpu `
     -p 8080:8080 `
     -v ${env:USERPROFILE}\nextplaid-data:/data/indices `
     -v ${env:USERPROFILE}\.cache\huggingface:/models `
     -e RUST_LOG=info `
     next-plaid-api:cuda-12.6 `
     --host 0.0.0.0 `
     --port 8080 `
     --index-dir /data/indices `
     --model lightonai/LateOn-Code `
     --cuda `
     --batch-size 64 `
     --parallel 4
   ```

2. **Use Docker Compose with CUDA 12.6:**
   ```powershell
   docker-compose -f docker-compose.yml -f docker-compose.cuda126.yml up -d
   ```

## Building Custom CUDA 12.6 Image

For Windows 11 with NVIDIA driver 551+ or Linux with driver 550+, you may need to build a custom image with CUDA 12.6:

### Prerequisites
- Docker Desktop with WSL2 backend (Windows)
- NVIDIA Container Toolkit
- Git for Windows

### Build Steps

```powershell
# 1. Clone your fork
git clone git@github.com:abhishekbhakat/next-plaid.git
cd next-plaid

# 2. Build the image (this may take 10-20 minutes)
docker build -t next-plaid-api:cuda-12.6 `
  -f next-plaid-api/Dockerfile.cuda126 `
  --target runtime-cuda .

# 3. Run the container
docker run --gpus all -d `
  --name nextplaid-gpu `
  -p 8080:8080 `
  -v ${env:USERPROFILE}\nextplaid-data:/data/indices `
  -v ${env:USERPROFILE}\.cache\huggingface:/models `
  -e RUST_LOG=info `
  next-plaid-api:cuda-12.6 `
  --host 0.0.0.0 `
  --port 8080 `
  --index-dir /data/indices `
  --model lightonai/LateOn-Code `
  --cuda `
  --batch-size 64 `
  --parallel 4
```

### Verify Build

```powershell
# Check container is running
docker ps

# Check health
curl http://localhost:8080/health

# Check GPU is being used
docker exec nextplaid-gpu nvidia-smi
```

## Performance Tips

### NVIDIA 4070 Super (Linux/macOS Server)

```bash
# Optimal settings for 12GB VRAM
docker run --gpus all -d \
  --name nextplaid-gpu \
  -p 8080:8080 \
  -v ~/nextplaid-data:/data/indices \
  ghcr.io/lightonai/next-plaid:cuda-1.0.8 \
  --host 0.0.0.0 \
  --port 8080 \
  --index-dir /data/indices \
  --model lightonai/LateOn-Code \
  --cuda \
  --batch-size 64 \
  --parallel 4 \
  --int8  # Use INT8 for 2x speedup
```

### NVIDIA 4070 Super (Windows 11 Server)

```powershell
# Optimal settings for 12GB VRAM on Windows
docker run --gpus all -d `
  --name nextplaid-gpu `
  -p 8080:8080 `
  -v ${env:USERPROFILE}\nextplaid-data:/data/indices `
  ghcr.io/lightonai/next-plaid:cuda-1.0.8 `
  --host 0.0.0.0 `
  --port 8080 `
  --index-dir /data/indices `
  --model lightonai/LateOn-Code `
  --cuda `
  --batch-size 64 `
  --parallel 4 `
  --int8
```

### Expected Performance on 4070 Super

- **Encoding:** ~100-200 docs/sec (with --int8)
- **Search:** ~50-100 QPS
- **Latency:** ~50-100ms per query
