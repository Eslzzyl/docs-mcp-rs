[English](./README.md) | [中文](./README.zh.md)

---

# docs-mcp-rs

[![Rust](https://img.shields.io/badge/Rust-2024-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A documentation indexing and search MCP (Model Context Protocol) server implemented in Rust. It can scrape, index documentation websites, and provide intelligent search capabilities to AI assistants via the MCP protocol.

## Features

- **Hybrid Search** - Combines vector search and full-text search (FTS5) for more accurate results
- **Intelligent Scraping** - Supports headless browser rendering to scrape JavaScript-rendered documentation sites
- **Multi-Version Management** - Supports independent indexing and management of multiple versions of the same library
- **MCP Protocol** - Supports both HTTP and stdio transport modes, can be directly invoked by AI tools like Claude and Cursor
- **Web Interface** - Built-in Web UI with Chinese and English language support for easy management and search
- **High Performance** - Implemented in Rust with low memory footprint and fast speed
- **Multiple Embedding Models** - Supports OpenAI and Google embedding models with configurable custom API endpoints

## Installation

### Build from Source

```bash
git clone https://github.com/Eslzzyl/docs-mcp-rs.git
cd docs-mcp-rs
cargo build --release
```

The compiled binary is located at `target/release/docs-mcp-rs`.

## Quick Start

### 1. Configure Environment Variables

Copy the example configuration file:

```bash
cp .env.example .env
```

Edit the `.env` file to configure your API keys:

```env
# OpenAI API configuration
OPENAI_API_KEY=sk-xxxxxxx
OPENAI_API_BASE=https://api.openai.com/v1

# Embedding model (format: provider:model)
DOCS_MCP_EMBEDDING_MODEL=openai:text-embedding-3-small
```

### 2. Start the Server

HTTP mode (default):

```bash
docs-mcp-rs serve
```

The server will start at `http://localhost:26301`. Visit `http://localhost:26301` to open the Web UI.

stdio mode (for MCP client integration):

```bash
docs-mcp-rs serve --stdio
```

### 3. Index Documentation

Use CLI to scrape documentation:

```bash
# Index React documentation
docs-mcp-rs scrape react https://react.dev --version 18

# Index Vue documentation with max pages limit
docs-mcp-rs scrape vue https://vuejs.org/guide --version 3 --max-pages 500
```

### 4. Search Documentation

```bash
# Search indexed documentation
docs-mcp-rs search react "how to use hooks"

# Search in a specific version
docs-mcp-rs search vue "computed properties" --version 3
```

## CLI Commands

### `serve` - Start MCP Server

```bash
docs-mcp-rs serve [OPTIONS]

Options:
  -p, --port <PORT>  HTTP server port [default: 26301]
      --stdio        Run in stdio mode
```

### `scrape` - Scrape and Index Documentation

```bash
docs-mcp-rs scrape <LIBRARY> <URL> [OPTIONS]

Arguments:
  <LIBRARY>  Library name
  <URL>      Starting URL

Options:
  -v, --version <VERSION>      Version number
  -p, --max-pages <MAX_PAGES>  Maximum pages to scrape [default: 1000]
  -d, --max-depth <MAX_DEPTH>  Maximum crawl depth [default: 3]
  -c, --concurrency <NUM>      Concurrency number [default: 5]
```

### `search` - Search Documentation

```bash
docs-mcp-rs search <LIBRARY> <QUERY> [OPTIONS]

Arguments:
  <LIBRARY>  Library name
  <QUERY>    Search query

Options:
  -v, --version <VERSION>  Version number
  -l, --limit <LIMIT>      Result limit [default: 5]
```

### `list` - List Indexed Libraries

```bash
docs-mcp-rs list
```

### `remove` - Remove Index

```bash
docs-mcp-rs remove <LIBRARY> [OPTIONS]

Arguments:
  <LIBRARY>  Library name

Options:
  -v, --version <VERSION>  Version number (removes entire library if not specified)
```

## MCP Tools

This server provides the following MCP tools for AI assistants:

| Tool Name | Description |
|-----------|-------------|
| `scrape_docs` | Scrape and index documentation websites |
| `search_docs` | Search indexed documentation |
| `list_libraries` | List all indexed libraries |
| `remove_library` | Remove indexed libraries |

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENAI_API_KEY` | OpenAI API key | - |
| `OPENAI_API_BASE` | OpenAI API base URL | `https://api.openai.com/v1` |
| `GOOGLE_API_KEY` | Google API key | - |
| `GOOGLE_API_BASE` | Google API base URL | - |
| `DOCS_MCP_EMBEDDING_MODEL` | Embedding model (`provider:model` format) | `openai:text-embedding-3-small` |
| `DOCS_MCP_EMBEDDING_DELAY_MS` | Delay between embedding requests (ms) | 150 |
| `DOCS_MCP_EMBEDDING_MAX_RPM` | Max requests per minute | 1800 |
| `DOCS_MCP_EMBEDDING_MAX_TPM` | Max tokens per minute | 800000 |
| `DOCS_MCP_EMBEDDING_MAX_RETRIES` | Max retries for 429 errors | 3 |
| `DOCS_MCP_EMBEDDING_RETRY_BASE_DELAY_MS` | Base retry delay (ms) | 1000 |

## Technical Architecture

- **Web Framework**: Axum
- **Database**: SQLite + sqlite-vec (vector extension)
- **MCP Protocol**: rmcp
- **Embedding APIs**: async-openai, reqwest
- **HTML Parsing**: scraper, fast_html2md
- **Browser Automation**: headless_chrome

## Project Structure

```
docs-mcp-rs/
├── src/
│   ├── cli/          # Command-line argument parsing
│   ├── core/         # Core configuration and types
│   ├── embed/        # Embedding model integration
│   ├── events/       # Event bus
│   ├── mcp/          # MCP server implementation
│   ├── pipeline/     # Scraping pipeline management
│   ├── scraper/      # Web scraping
│   ├── splitter/     # Document chunking
│   ├── store/        # Data storage
│   └── web/          # Web interface
├── migrations/       # Database migrations
├── public/           # Static assets
└── data/             # Data directory
```

## Related Projects

- [docs-mcp-server](https://github.com/arabold/docs-mcp-server) - Original TypeScript/Node.js implementation

## License

MIT License
