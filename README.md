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

### Release Binaries

Tarballs are available on the [Github release page](https://github.com/tarka/netlink-ddns/releases). 
These contain binaries, documentation, example configuration files, and an example
systemd configuration.

### Install from crates.io

```bash
cargo install netlink-ddns
```

### Building from Source

You'll need Rust installed to build the project:

```bash
# Clone the repository
git clone https://github.com/tarka/netlink-ddns.git
cd netlink-ddns

# Build & install the project
cargo build --release

# The binary will be in target/release/netlink-ddns
```

## Configuration

The service is configured using a [Corn](https://cornlang.dev/) config file. By
default, it looks for the configuration at `/etc/netlink-ddns/config.corn`.

Example configuration:

```
let {
  // Secrets can be stored in environment variables. The systemd service can set these
  // from a secrets file.
  $env_PORKBUN_KEY = "a_key"
  $env_PORKBUN_SECRET = "a_secret"

}  in {

  log_level = "debug"
  iface = "test0"

  ddns = {
    domain = "example.com"
    host = "test"
    provider = {
      name = "porkbun"
      key = $env_PORKBUN_KEY
      secret = $env_PORKBUN_SECRET
    }
  }
}
```

## Usage

### Running as a Service

The file `systemd/netlink-ddns.service` contains an example systemd
configuration. This is also available in the release tarballs. 

## Requirements

- Linux system with netlink support
- Network interface with dynamic IP address
- A DNS account with API access
- systemd (for service integration)

## License

This project is licensed under the GPL 3.0 - see the LICENSE file for details.
