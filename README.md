# Netlink DDNS

A dynamic DNS (DDNS) updater for Linux that monitors network interface changes
via netlink and automatically updates DNS records.

## Overview

Netlink DDNS is a lightweight service that monitors your network interfaces via
kernel netlink for IP address changes and automatically updates your DNS records
to match your current IP address. Currently only Gandi is supported as a
upstream provider, but more can be added.

## Features

- Real-time monitoring of network interface changes using netlink sockets
- Automatic DNS record updates for Gandi's LiveDNS API
- systemd service integration for easy deployment
- Support for both API keys and Personal Access Tokens (PAT) for Gandi authentication
- Dry-run mode for testing configurations
- Comprehensive logging with configurable log levels
- Written in Rust for performance and safety

## Installation

### Building from Source

You'll need Rust installed to build the project:

```bash
# Clone the repository
git clone https://github.com/yourusername/netlink-ddns.git
cd netlink-ddns

# Build & install the project
cargo install --path .

# The binary will be in ~/.cargo/bin/
```

### Installing as a System Service

1. Copy the binary to a system location:
   ```bash
   sudo cp target/release/netlink-ddns /usr/local/bin/
   ```

2. Create the configuration directory and file:
   ```bash
   sudo mkdir -p /etc/netlink-ddns
   sudo cp tests/config.toml /etc/netlink-ddns/config.toml
   sudo chown www-data:www-data /etc/netlink-ddns/config.toml
   ```

3. Edit the configuration file:
   ```bash
   sudo nano /etc/netlink-ddns/config.toml
   ```

4. Install the systemd service:
   ```bash
   sudo cp systemd/netlink-ddns.service /etc/systemd/system/
   sudo systemctl daemon-reload
   ```

5. Start and enable the service:
   ```bash
   sudo systemctl start netlink-ddns
   sudo systemctl enable netlink-ddns
   ```

## Configuration

The service is configured using a TOML file. By default, it looks for the configuration at `/etc/netlink-ddns/config.toml`, but you can specify a different location using the `NLDDNS_CONFIG` environment variable.

Example configuration:

```toml
# Optional log level (default: INFO)
log_level = "info"

# Gandi API key (either gandi_api_key OR gandi_pat_key is required)
gandi_api_key = "your-api-key-here"

# OR Gandi Personal Access Token (alternative to API key)
# gandi_pat_key = "your-pat-key-here"

# Domain name to update
domain = "example.com"

# Host name to update (e.g., "www" for www.example.com)
host = "home"

# Network interface to monitor
iface = "eth0"

# Optional dry-run mode (default: false)
# dry_run = true
```

### Configuration Options

- `log_level`: Optional logging level (trace, debug, info, warn, error)
- `gandi_api_key`: Your Gandi API key (required unless using PAT)
- `gandi_pat_key`: Your Gandi Personal Access Token (alternative to API key)
- `domain`: The domain name to update
- `host`: The host/subdomain to update
- `iface`: The network interface to monitor for IP changes
- `dry_run`: If true, logs what would be done without actually updating DNS

## Usage

### Running Manually

```bash
# With default configuration path
netlink-ddns

# With custom configuration path
NLDDNS_CONFIG=/path/to/your/config.toml netlink-ddns
```

### Running as a Service

```bash
# Start the service
sudo systemctl start netlink-ddns

# Stop the service
sudo systemctl stop netlink-ddns

# Check service status
sudo systemctl status netlink-ddns

# View service logs
sudo journalctl -u netlink-ddns -f
```

## Requirements

- Linux system with netlink support
- Network interface with dynamic IP address
- Gandi account with API access
- systemd (for service integration)

## License

This project is licensed under the GPL 3.0 - see the LICENSE file for details.
