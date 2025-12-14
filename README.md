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
- Uses the [zone-update](https://github.com/tarka/zone-update/) library to allow
  DNS updates for multiple DNS providers.

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
   sudo cp examples/config.corn /etc/netlink-ddns/config.corn
   sudo chown www-data:www-data /etc/netlink-ddns/config.corn
   ```

3. Edit the configuration file:
   ```bash
   sudo nano /etc/netlink-ddns/config.corn
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

The service is configured using a [Corn](https://cornlang.dev/) config file. By
default, it looks for the configuration at `/etc/netlink-ddns/config.corn`.

Example configuration:

```
let {
  // These can also be environment variables
  $porkbun_key = "a_key"
  $porkbun_secret = "a_secret"

}  in {

  log_level = "debug"
  iface = "test0"

  ddns = {
    provider = {
      name = "porkbun"
      key = $porkbun_key
      secret = $porkbun_secret
    }

    domain = "example.com"
    host = "test"
  }
}
```

## Usage

### Running Manually

```bash
# With default configuration path
netlink-ddns

# With custom configuration path
NLDDNS_CONFIG=/path/to/your/config.corn netlink-ddns
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
- A DNS account with API access
- systemd (for service integration)

## License

This project is licensed under the GPL 3.0 - see the LICENSE file for details.
