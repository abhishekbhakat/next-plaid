# semgrep

Simple semantic code search using a remote NextPlaid API server.

## Overview

`semgrep` is a lightweight CLI tool for semantic code search. It parses your codebase locally, sends text to a remote GPU server for embedding, and performs fast semantic search using the NextPlaid API.

## Installation

### Build from Source

```bash
cd semgrep
cargo build --release
```

The binary will be at `./target/release/semgrep`.

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SEMGREP_URL` | URL of the NextPlaid API server | `http://127.0.0.1:8080` |
| `SEMGREP_INDEX` | Optional index name override | Auto-generated from path |

### Setting the Server URL

**Option 1: Environment Variable**

```bash
export SEMGREP_URL=http://your-server-ip:8080
semgrep "database connection" ./src
```

**Option 2: One-time Use**

```bash
SEMGREP_URL=http://your-server-ip:8080 semgrep "error handling" ./src
```

**Option 3: CLI Flag**

```bash
semgrep --url http://your-server-ip:8080 "authentication" ./src
```

## Usage

### Basic Search

```bash
# Search in current directory
semgrep "error handling"

# Search in specific directory
semgrep "database connection" ./src

# Search with more results
semgrep "config parser" -n 20
```

### Filter by Language

```bash
# Search only Rust files
semgrep "async function" --lang rust

# Search only Python files
semgrep "decorator" --lang python
```

### Filter by Extension

```bash
semgrep "http request" --ext .rs
```

### Output Options

```bash
# JSON output for scripting
semgrep "api endpoint" --json

# List files only (like grep -l)
semgrep "todo" -l

# Show content snippets
semgrep "main function" -c
```

### Interactive Mode

```bash
semgrep -i
```

This starts an interactive session where you can type queries:

```
semgrep> database connection
semgrep> error handling
semgrep> quit
```

## Examples

```bash
# Find authentication-related code
semgrep "user authentication" --lang rust

# Find database queries
semgrep "sql query" ./backend --ext .py

# Find configuration parsing
semgrep "parse config" -n 15

# Export results to JSON
semgrep "http client" --json > results.json
```

## How It Works

1. **Parse**: Tree-sitter parses your code locally (functions, classes, methods)
2. **Index**: Code is sent to the remote server for embedding and indexing
3. **Search**: Queries are encoded and searched using MaxSim scoring
4. **Results**: Ranked results with file paths, line numbers, and similarity scores

## Requirements

- Remote NextPlaid API server with GPU support
- Rust 1.70+ (for building)
- Network access to the server

## See Also

- [WSL2 Setup Guide](../WSL2_SETUP.md) - For running the server on Windows with WSL2
- [NextPlaid API Documentation](../next-plaid-api/README.md)
