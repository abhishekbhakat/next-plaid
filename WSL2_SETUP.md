# WSL2 Setup Guide for NextPlaid GPU Server

This guide explains why and how to run the NextPlaid GPU server in WSL2 (Windows Subsystem for Linux) instead of Docker Desktop.

## Why WSL2?

### The Problem: ONNX Runtime + Docker Desktop

When running the NextPlaid API container with GPU support in **Docker Desktop on Windows**, you may encounter this error:

```
CUDA failure 35: CUDA driver version is insufficient for CUDA runtime version
Discovered OrtHardwareDevice {vendor_id:0x1022, device_id:0x0, vendor:AMD, type:0}
```

**Root Cause:**

- ONNX Runtime uses a specific GPU discovery mechanism that doesn't work well with Docker Desktop's WSL2 backend
- Docker Desktop runs containers in a WSL2 VM, but the GPU passthrough doesn't properly expose NVIDIA GPUs to ONNX Runtime
- ONNX Runtime detects an AMD device (your CPU's iGPU or a virtual device) instead of the NVIDIA GPU

### The Solution: Native WSL2

Running Docker **directly in WSL2** (not Docker Desktop) provides:

- Direct access to NVIDIA GPUs via the NVIDIA Container Toolkit
- Proper GPU detection by ONNX Runtime
- Better CUDA compatibility

## Setup Steps

### 1. Install WSL2 with Debian/Ubuntu

```powershell
# In Windows PowerShell as Administrator
wsl --install -d Debian
# Restart your computer, then set up your Debian user
```

### 2. Install NVIDIA Container Toolkit in WSL2

Follow the official guide: <https://docs.nvidia.com/cuda/wsl-user-guide/index.html>

Or run these commands in WSL2:

```bash
# Install Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
sudo usermod -aG docker $USER

# Install NVIDIA Container Toolkit
distribution=$(. /etc/os-release;echo $ID$VERSION_ID)
curl -s -L https://nvidia.github.io/nvidia-docker/gpgkey | sudo apt-key add -
curl -s -L https://nvidia.github.io/nvidia-docker/$distribution/nvidia-docker.list | sudo tee /etc/apt/sources.list.d/nvidia-docker.list
sudo apt update
sudo apt install -y nvidia-docker2
sudo systemctl restart docker
```

### 3. Verify GPU Access

```bash
# Should show your NVIDIA GPUs
docker run --rm --gpus all nvidia/cuda:12.6.0-base-ubuntu22.04 nvidia-smi
```

### 4. Build and Run NextPlaid

```bash
# Clone the repository
cd ~
git clone git@github.com:abhishekbhakat/next-plaid.git
cd next-plaid

# Build the CUDA 12.6 image
make docker-build-cuda-12-6

# Run with specific GPU (e.g., GPU 1 = RTX 3060)
make docker-up-cuda-12-6 GPU_DEVICE=1
```

## Port Forwarding (Windows → WSL2)

By default, WSL2 containers are only accessible from the Windows host, not from other machines on your network. To access the server from your MacBook or other devices:

### Option 1: Automatic Port Forwarding (Recommended)

Run these commands in **Windows PowerShell as Administrator**:

```powershell
# Forward port 8080 from Windows to WSL2
netsh interface portproxy add v4tov4 listenport=8080 listenaddress=0.0.0.0 connectport=8080 connectaddress=127.0.0.1

# Open Windows Firewall
netsh advfirewall firewall add rule name="NextPlaid API" dir=in action=allow protocol=tcp localport=8080

# Verify the port proxy
netsh interface portproxy show all
```

Now you can access the server from any machine on your network:

```
http://<windows-ip>:8080
```

### Option 2: SSH Tunnel (Alternative)

From your MacBook or other machine:

```bash
ssh -L 8080:localhost:8080 user@<windows-ip>
```

Then access via `http://localhost:8080`

## Troubleshooting

### Permission Denied on /models

If you see:

```
mkdir: cannot create directory '/models/GTE-ModernColBERT-v1': Permission denied
```

Fix the ownership:

```bash
# In WSL2
sudo chown -R $(id -u):$(id -g) ~/.local/share/next-plaid ~/.cache/huggingface
```

### Container Can't See GPU

Verify NVIDIA Container Toolkit is installed:

```bash
docker run --rm --gpus all nvidia/cuda:12.6.0-base-ubuntu22.04 nvidia-smi
```

If this fails, reinstall the NVIDIA Container Toolkit.

### Port Forwarding Not Working

Check if the port is listening:

```powershell
# In Windows PowerShell
netsh interface portproxy show all
netstat -an | findstr 8080
```

Delete and recreate the port proxy:

```powershell
netsh interface portproxy delete v4tov4 listenport=8080 listenaddress=0.0.0.0
netsh interface portproxy add v4tov4 listenport=8080 listenaddress=0.0.0.0 connectport=8080 connectaddress=127.0.0.1
```

## Summary

| Component | Purpose |
|-----------|---------|
| **WSL2** | Linux environment with native GPU access |
| **Docker in WSL2** | Container runtime with proper NVIDIA support |
| **NVIDIA Container Toolkit** | GPU passthrough for Docker containers |
| **netsh portproxy** | Forward Windows ports to WSL2 |

## Quick Reference

```bash
# Start server on GPU 1 (RTX 3060)
make docker-up-cuda-12-6 GPU_DEVICE=1

# Start server on GPU 0 (RTX 4070 Super)
make docker-up-cuda-12-6 GPU_DEVICE=0

# Stop server
make docker-down

# View logs
docker logs next-plaid-next-plaid-api-1 -f
```

## References

- [NVIDIA WSL2 User Guide](https://docs.nvidia.com/cuda/wsl-user-guide/index.html)
- [NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/install-guide.html)
- [Docker Desktop WSL2 Backend](https://docs.docker.com/desktop/wsl/)
