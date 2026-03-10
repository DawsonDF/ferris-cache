# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build

# Run the server
cargo run

# Check for compilation errors without building
cargo check

# Run tests
cargo test

# Run a single test by name
cargo test <test_name>

# Lint
cargo clippy

# Format
cargo fmt

# Test all commands (server must be running)
printf "SET name ferris\r\nGET name\r\nDELETE name\r\nGET name\r\n" | nc 127.0.0.1 7878
# Expected: OK / ferris / OK / NIL

# Multi-word value
printf "SET msg hello world\r\nGET msg\r\n" | nc 127.0.0.1 7878
# Expected: OK / hello world

# Unknown command
printf "PING\r\n" | nc 127.0.0.1 7878
# Expected: ERR unknown command

# Docker build + run
docker build -t ferris-cache:latest .
docker run -p 7878:7878 ferris-cache:latest
```

## Architecture

`ferris-cache` is a minimal Redis-like TCP key-value cache server written in Rust, listening on `0.0.0.0:7878`.

**Implementation:**
- `src/main.rs` is the only source file — all logic lives here
- Async I/O via `tokio`; one `tokio::spawn` per TCP connection
- Shared backing store: `Arc<DashMap<String, String>>` — lock-free concurrent HashMap
- Each connection loops on `read_line`, handling multiple commands per connection
- `splitn(3, ' ')` preserves spaces in values (e.g. `SET msg hello world`)

**Configuration:**
- `FERRIS_BIND` — bind address (default: `0.0.0.0:7878`)

**Command protocol:** Plain text over TCP, line-oriented (`\r\n` terminated):

| Command | Response |
|---|---|
| `SET <key> <value>` | `OK` |
| `GET <key>` | `<value>` or `NIL` |
| `DELETE <key>` | `OK` or `NIL` |
| `FLUSH` | `OK` |
| anything else | `ERR unknown command` |
