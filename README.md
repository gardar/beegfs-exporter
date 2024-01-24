# BeeGFS Exporter

The BeeGFS Exporter is a simple Rust program designed to export performance metrics from a BeeGFS instance to Prometheus. This exporter provides a convenient way to monitor and collect performance data for your BeeGFS setup.

## Usage

```bash
beegfs-exporter [OPTIONS]
```

Options:
- -c, --config-file <CONFIG_FILE>: Path to the BeeGFS configuration file.
- -b, --bind-address <BIND_ADDRESS>: Port to run the exporter on.
- -v, --verbose: Enable verbose mode.
- -r, --restart-attempts <RESTART_ATTEMPTS>: Maximum number of crashes before giving up (default: 10).
- -h, --help: Print help.

Default endpoint (set with `-b` for example `0.0.0.0:9091`):

```
curl http://localhost:13337/metrics
```


## Getting Started

### Prerequisites

Before using the BeeGFS Exporter, make sure you have Rust installed on your system. You can install Rust by following the instructions on the official Rust website.

### Installation

Clone the repository:

```bash
git clone https://github.com/besnardjb/beegfs-exporter.git
```

Build the project:

```bash
cd beegfs-exporter
cargo build --release
```

### Usage Examples

Run the BeeGFS Exporter with default settings:

```bash
./target/release/beegfs-exporter
```

Specify the configuration file and bind address:

```bash
./target/release/beegfs-exporter -c /path/to/beegfs.conf -b 127.0.0.1:9090
```

Enable verbose mode:

```bash
./target/release/beegfs-exporter -v
```

## Contributing

If you find any issues or have suggestions for improvements, feel free to open an issue or create a pull request.

## License

This project is licensed under the CECILL-C (LGPL compatible) License.