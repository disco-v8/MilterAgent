# MilterAgent

A high-performance email filtering server implementing the Milter protocol, designed to protect against phishing emails from spoofed legitimate companies and organizations.

## Overview

MilterAgent is a comprehensive email security solution that implements the Milter protocol for real-time email filtering. Built with Rust and Tokio for maximum performance, it provides advanced phishing detection capabilities while maintaining low latency and high throughput.

## Features

- **Advanced Phishing Detection**: Comprehensive filters for detecting phishing emails impersonating:
  - Banks and financial institutions (MUFG, Mizuho, SMBC, etc.)
  - Delivery and logistics companies (Japan Post, Yamato, Sagawa, DHL, etc.)
  - Transportation companies (JR, airlines, private railways, etc.)
  - E-commerce platforms and online services
- **Milter Protocol Support**: Full implementation of the Milter protocol for seamless integration
- **Real-time Processing**: Asynchronous Rust implementation for handling thousands of concurrent connections
- **High-Speed Parallel Filtering**: Multi-threaded filter evaluation with parallel execution for maximum performance
- **Flexible Configuration**: Modular configuration system with include support
- **Signal Handling**: Graceful shutdown and live configuration reload via SIGHUP/SIGTERM
- **Cross-platform**: Supports both Unix and Windows environments
- **Comprehensive Logging**: Configurable log levels with JST timestamp support

## Installation

### Prerequisites

## Architecture

- **Asynchronous Runtime**: Built with Tokio for handling multiple concurrent connections
- **Modular Design**: Separate modules for parsing, filtering, and protocol handling
- **Memory Safe**: Written in Rust for guaranteed memory safety and performance
- **Parallel Filter Engine**: Configurable rule-based filtering system with regex support and multi-threaded parallel processing for high throughput

## Installation

### Prerequisites

- Rust 1.70 or later
- Cargo package manager
- Compatible mail server (Postfix, Sendmail, etc.)

### Building from Source

```bash
cd /usr/src/
git clone https://github.com/disco-v8/MilterAgent.git
mv MilterAgent "MilterAgent-$(date +%Y%m%d)"
cd /usr/src/"MilterAgent-$(date +%Y%m%d)"

cargo build --release

/bin/cp -af ./target/release/milter_agent /usr/sbin/
rsync -av ./logrotate.d/milter_agent /etc/logrotate.d/
rsync -av ./systemd/milter_agent.service /usr/lib/systemd/system/
rsync -av ./etc/ /etc/
```

## Configuration

The server uses a modular configuration system:

### Main Configuration File

Create `MilterAgent.conf`:

```
Listen 127.0.0.1:8898
Client_timeout 30
Log_file /var/log/milteragent.log
Log_level info

include MilterAgent.d
```

### Filter Configuration

Place filter files in the `MilterAgent.d/` directory:

- `filter_bank.conf` - Banking and financial services filters
- `filter_transport.conf` - Transportation and logistics filters
- Additional filter files as needed

If you want to start with WARN (adding only a header) instead of REJECT, please perform the following batch replacement.

```
chmod 700 /etc/MilterAgent.d/reject2warn.sh
cd /etc/MilterAgent.d/
./reject2warn.sh
```

### Filter Syntax

```
filter[filter_name] = 
    "decode_from:(?i)(Pattern):AND",
    "decode_from:!@(.*\.)?legitimate-domain\.com:REJECT"
```

### Configuration Options

- `Listen`: Server bind address and port
  - Format: `IP:PORT` or just `PORT`
  - Example: `127.0.0.1:8898` or `8898`
- `Client_timeout`: Client inactivity timeout in seconds
- `Log_file`: Path to log file (optional, defaults to stdout)
- `Log_level`: Logging verbosity (`info`, `trace`, `debug`)
- `include`: Include additional configuration directories

## Usage

### Starting the Server

```bash
systemctl daemon-reload
systemctl --no-pager status milter_agent

systemctl enable milter_agent
systemctl --no-pager status milter_agent

systemctl start milter_agent
systemctl --no-pager status milter_agent
```

### Mail Server Integration

#### Postfix Integration

Add to `/etc/postfix/main.cf`:

```
smtpd_milters = inet:127.0.0.1:8898
milter_default_action = accept
milter_protocol = 6
```

Restart Postfix:

```bash
sudo systemctl restart postfix
```

#### Sendmail Integration

Add to `/etc/mail/sendmail.mc`:

```
INPUT_MAIL_FILTER(`milteragent', `S=inet:8898@127.0.0.1')
```

### Signal Handling

- **SIGHUP**: Reload configuration files
- **SIGTERM**: Graceful shutdown
- **Ctrl+C** (Windows): Graceful shutdown

```bash
# Reload configuration
systemctl reload milter_agent
systemctl --no-pager status milter_agent

# Graceful shutdown
systemctl stop milter_agent
systemctl --no-pager status milter_agent
```

## Logging

The server supports configurable log levels:

- `info` (0): Basic operational messages
- `trace` (2): Detailed tracing information  
- `debug` (8): Comprehensive debugging output

Logs include JST timestamps and are output to either a file or stdout based on configuration.

## Filter Examples

### Banking Filter

```
filter[phish_mufg] = 
    "decode_from:(?i)(三菱UFJ|MUFG):AND",
    "decode_from:!@(.*\.)?mufg\.jp:REJECT"
```

### Transportation Filter

```
filter[phish_jreast] = 
    "decode_from:(?i)(JR東日本|East Japan Railway):AND",
    "decode_from:!(@(.*\.)?jreast\.co\.jp|@(.*\.)?jre-vts\.com):REJECT"
```

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Security Notice

This software is designed to help protect against phishing emails but should be used as part of a comprehensive email security strategy. Regular updates to filter rules are recommended to address new phishing campaigns.

## Support

For issues, questions, or contributions, please use the GitHub issue tracker.
```

### Multi-part Email with Attachments

```
[2025/07/23 15:31:20] このメールはマルチパートです
[2025/07/23 15:31:20] [mail-parser] テキストパート数: 1
[2025/07/23 15:31:20] [mail-parser] 非テキストパート数: 1
[2025/07/23 15:31:20] 本文(1): Email body content
[2025/07/23 15:31:20] 非テキストパート(1): content_type="application/pdf", encoding=Base64, filename=document.pdf, size=1024 bytes
```

## Architecture

### Module Structure

- **main.rs**: Server startup, configuration management, signal handling
- **client.rs**: Per-client Milter protocol handling
- **milter.rs**: Milter command decoding and response generation
- **milter_command.rs**: Milter protocol command definitions
- **parse.rs**: MIME email parsing and output formatting
- **init.rs**: Configuration file management
- **logging.rs**: JST timestamp logging macros

### Milter Protocol Flow

1. **OPTNEG**: Protocol negotiation
2. **CONNECT**: Client connection information
3. **HELO/EHLO**: SMTP greeting
4. **DATA**: Macro information
5. **HEADER**: Email headers (multiple)
6. **BODY**: Email body content (multiple chunks)
7. **BODYEOB**: End of body - triggers email parsing and output

## Dependencies

- [tokio](https://tokio.rs/): Asynchronous runtime
- [mail-parser](https://crates.io/crates/mail-parser): MIME email parsing
- [chrono](https://crates.io/crates/chrono): Date and time handling
- [chrono-tz](https://crates.io/crates/chrono-tz): Timezone support
- [lazy_static](https://crates.io/crates/lazy_static): Global static variables

## Development

### Running in Development Mode

```bash
cargo run
```

### Testing with Sample Email

You can test the server by sending emails through a configured Postfix instance or using telnet to send raw SMTP commands.

### Debug Features

- NUL byte visualization: `\0` bytes are displayed as `<NUL>`
- Hex dump output for unknown commands
- Detailed protocol command logging
- Error handling with descriptive messages

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

**Notice:**
- This project uses third-party crates, some of which are licensed under Apache-2.0.
- In particular, the [mail-parser](https://crates.io/crates/mail-parser) crate is licensed under Apache-2.0. Please review its license if you redistribute or modify this software.

## Support

For issues, questions, or contributions, please open an issue on GitHub.

## Changelog

### v0.1.0
- Initial release
- Basic Milter protocol implementation
- MIME email parsing and output
- Configuration file support
- Signal handling
- JST timestamp logging
