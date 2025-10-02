# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.3] - 2025-10-02

### Changed
- Optimized loopback address connection handling to avoid unnecessary processing
- Loopback connections (127.0.0.1, ::1) are now silently dropped without logging or spawning client threads

## [0.3.2] - 2025-09-18

### Added
- Japanese text sanitization and From header spoofing countermeasures
- Removal of invisible and bidirectional control characters in Japanese text
- Mitigation of address spoofing via From name manipulation

## [0.3.1] - 2025-08-20

### Fixed
- Mitigation of recursion depth limitations
- Modified processing to avoid recursion depth issues in fancy-regex

## [0.3.0] - 2025-08-17 - Semantic Filtering Reinforced

### Added
- Unicode normalization (NFKC) and invisible character stripping to Subject/From headers
- Improved spoofing resistance against obfuscated Unicode and bidirectional control characters
- Enhanced semantic matching accuracy and operational reproducibility

## [0.2.0] - 2025-08-08

### Added
- **Rust 2024 Edition Support**: Upgraded from Rust 2021 to 2024 edition for improved performance and latest language features
- **Advanced Regex Engine**: Migrated to `fancy-regex` 0.16 with support for:
  - Negative lookahead (`(?!...)`) and positive lookahead (`(?=...`)
  - Negative lookbehind (`(?<!...)`) and positive lookbehind (`(?<=...`)
  - Sophisticated phishing detection patterns
- **Spamhaus API Integration**: 
  - Automatic detection and reporting of phishing email source IPs
  - Configurable API authentication (`Spamhaus_api_token`)
  - Configurable API endpoint (`Spamhaus_api_url`)
  - Enable/disable reporting (`Spamhaus_report`)
  - Threat intelligence sharing for flagged emails
- Enhanced filter examples demonstrating negative lookahead patterns
- Comprehensive code documentation and comments
- Better error messages for invalid filter configurations

### Changed
- **Dependency Optimization**: 
  - Replaced `once_cell` with standard library `std::sync::OnceLock`
  - Removed unused `lazy_static` dependency
  - Updated all dependencies to latest compatible versions
- **Code Quality Improvements**:
  - Applied consistent formatting with `cargo fmt`
  - Resolved all `cargo clippy` warnings
  - Enhanced error handling in regex matching
  - Improved code readability and maintainability
- **Configuration System**: Enhanced regex compilation error reporting and configuration file parsing robustness

### Deprecated
- Support for Rust versions below 1.80 (required for Rust 2024 edition)

### Removed
- `lazy_static` dependency (unused)
- `once_cell` dependency (replaced with standard library)

### Added
- Enhanced phishing detection filters for major Japanese organizations:
  - Banking and financial services (MUFG, Mizuho, SMBC, etc.)
  - Transportation (JR companies, airlines, private railways)
  - Shipping companies (Japan Post, Yamato Transport, Sagawa Express, DHL)
  - E-commerce and online services
- Modular configuration system with include directory support
- Configurable log levels (info, trace, debug) with JST timestamps
- Signal handling for graceful shutdown and configuration reload
- Cross-platform support (Unix and Windows)
- GitHub Actions CI/CD pipeline
- Comprehensive documentation in English and Japanese

### Changed
- Simplified response packet building with standardized parameters
- Improved error handling and protocol compliance
- Enhanced logging system with level-based filtering

## [0.1.0] - 2025-07-23

### Added
- Initial release of MilterAgent
- Full Milter protocol implementation compatible with Postfix/Sendmail
- Asynchronous TCP server using Tokio runtime
- MIME email parsing with mail-parser crate
- Comprehensive email analysis and output:
  - From/To/Subject extraction
  - Content-Type and encoding detection
  - Multi-part email support
  - Attachment detection with filename extraction
  - Text/non-text part classification
- JST timestamp logging with chrono-tz
- Configuration file support (`MilterAgent.conf`)
- Signal handling:
  - SIGHUP for configuration reload
  - SIGTERM for graceful shutdown
- Debug features:
  - NUL byte visualization
  - Hex dump output for unknown commands
  - Detailed protocol logging
- Error handling and timeout management
- IPv4/IPv6 dual-stack support

### Technical Features
- Modular architecture with clear separation of concerns:
  - `main.rs`: Server startup and management
  - `client.rs`: Per-client Milter protocol handling
  - `milter.rs`: Milter command processing
  - `milter_command.rs`: Protocol definitions
  - `parse.rs`: Email parsing and analysis
  - `init.rs`: Configuration management
  - `logging.rs`: Timestamp logging utilities
- Comprehensive documentation and comments
- Rust 2021 edition compatibility
- MIT license

### Dependencies
- tokio 1.38 (async runtime)
- mail-parser 0.11 (MIME parsing)
- chrono 0.4 (date/time handling)
- chrono-tz 0.8 (timezone support)
- lazy_static 1.5.0 (global variables)
