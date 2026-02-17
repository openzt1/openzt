# OpenZT Instance Manager

A Docker-based instance management system for running Zoo Tycoon with OpenZT mod support.

## Features

- **API Server**: REST API for managing Zoo Tycoon Docker instances
- **CLI Client**: Command-line tool for interacting with the API
- **Auto Port Allocation**: Automatic allocation of RDP and console ports
- **Instance Lifecycle Management**: Create, list, get, delete instances
- **Log Access**: Retrieve logs from running instances
- **Health Monitoring**: Check API and instance status

## Prerequisites

- Docker (with daemon running)
- Rust/Cargo (for building from source)

## Quick Install

### From Source

```bash
# Clone the repository (if needed)
cd openzt-instance-manager

# Run the installer
./install.sh
```

The installer will:
1. Build the server and client binaries
2. Install them to `/usr/local/bin`
3. Create a systemd service
4. Set up the config directory

### Manual Install

```bash
# Build binaries
cargo build --release --bin openzt-instance-manager
cargo build --release --bin openzt --features cli

# Install binaries
sudo cp target/release/openzt-instance-manager /usr/local/bin/
sudo cp target/release/openzt /usr/local/bin/

# Create config directory
sudo mkdir -p /etc/openzt-instance-manager
sudo cp config.toml /etc/openzt-instance-manager/
```

## Configuration

Edit `/etc/openzt-instance-manager/config.toml`:

```toml
[server]
listen_address = "0.0.0.0:3000"

[ports]
rdp_start = 13390
rdp_end = 13490
console_start = 18081
console_end = 18181

[docker]
image = "finn/winezt:latest"
container_prefix = "openzt-"

[instances]
max_instances = 100
auto_cleanup_hours = 24
```

## Usage

### Starting the Server

```bash
# Start the service
sudo systemctl start openzt-instance-manager

# Enable on boot
sudo systemctl enable openzt-instance-manager

# Check status
sudo systemctl status openzt-instance-manager

# View logs
sudo journalctl -u openzt-instance-manager -f
```

### Using the CLI

```bash
# Check API health
openzt health

# List all instances
openzt list

# Create a new instance
openzt create /path/to/openzt.dll

# Get instance details
openzt get <instance-id>

# Get instance logs
openzt logs <instance-id>

# Delete an instance
openzt delete <instance-id>

# JSON output
openzt list --output json

# Custom API URL
openzt --api-url http://localhost:3000 list
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `health` | Check API server health |
| `list` | List all instances |
| `get <id>` | Get instance details |
| `create <dll>` | Create new instance |
| `logs <id>` | Get instance logs |
| `delete <id>` | Delete an instance |

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| GET | `/api/instances` | List all instances |
| POST | `/api/instances` | Create new instance |
| GET | `/api/instances/:id` | Get instance details |
| DELETE | `/api/instances/:id` | Delete instance |
| GET | `/api/instances/:id/logs` | Get instance logs |

## Create Instance Request

```json
{
  "openzt_dll": "<base64-encoded-dll>",
  "mods": [],
  "config": {
    "rdp_password": "optional-password"
  }
}
```

## Instance States

- **creating**: Container is being created
- **running**: Instance is running
- **stopped**: Instance is stopped
- **error**: An error occurred (see status message)

## Uninstall

```bash
./uninstall.sh
```

## Troubleshooting

### Server won't start
```bash
# Check Docker is running
sudo systemctl status docker

# Check for port conflicts
sudo netstat -tulpn | grep 3000

# View server logs
sudo journalctl -u openzt-instance-manager -n 50
```

### Container creation fails
```bash
# Verify Docker image
docker images | grep winezt

# Check Docker logs
docker logs <container-id>
```

### Port conflicts
The default port ranges are:
- RDP: 13390-13490
- Console: 18081-18181

Edit `/etc/openzt-instance-manager/config.toml` if you need different ports.

## Development

```bash
# Build both binaries
cargo build --release

# Run server directly
cargo run --bin openzt-instance-manager

# Run CLI
cargo run --bin openzt --features cli -- health
```
