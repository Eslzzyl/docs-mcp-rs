# docs-mcp-rs

[![Rust](https://img.shields.io/badge/Rust-2024-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

一个用 Rust 实现的文档索引和搜索 MCP (Model Context Protocol) 服务器。可以将文档网站爬取、索引，并通过 MCP 协议提供给 AI 助手进行智能搜索。

## 功能特性

- **混合搜索** - 结合向量搜索和全文搜索 (FTS5)，提供更精准的搜索结果
- **智能爬取** - 支持无头浏览器渲染，可爬取 JavaScript 渲染的文档网站
- **多版本管理** - 支持同一库的多个版本独立索引和管理
- **MCP 协议** - 支持 HTTP 和 stdio 两种传输模式，可被 Claude、Cursor 等 AI 工具直接调用
- **Web 界面** - 内置 Web UI，支持中英文界面，方便管理和搜索
- **高性能** - Rust 实现，内存占用低，速度快
- **多嵌入模型** - 支持 OpenAI 和 Google 嵌入模型，可配置自定义 API 端点

## 安装

### 从源码构建

```bash
git clone https://github.com/Eslzzyl/docs-mcp-rs.git
cd docs-mcp-rs
cargo build --release
```

编译后的二进制文件位于 `target/release/docs-mcp-rs`。

## 快速开始

### 1. 配置环境变量

复制示例配置文件：

```bash
cp .env.example .env
```

编辑 `.env` 文件，配置你的 API 密钥：

```env
# OpenAI API 配置
OPENAI_API_KEY=sk-xxxxxxx
OPENAI_API_BASE=https://api.openai.com/v1

# 嵌入模型 (格式: provider:model)
DOCS_MCP_EMBEDDING_MODEL=openai:text-embedding-3-small
```

### 2. 启动服务器

HTTP 模式（默认）：

```bash
docs-mcp-rs serve
```

服务器将在 `http://localhost:26301` 启动，访问 `http://localhost:26301` 打开 Web 界面。

stdio 模式（用于 MCP 客户端集成）：

```bash
docs-mcp-rs serve --stdio
```

### 3. 索引文档

使用 CLI 爬取文档：

```bash
# 索引 React 文档
docs-mcp-rs scrape react https://react.dev --version 18

# 索引 Vue 文档，限制最大页面数
docs-mcp-rs scrape vue https://vuejs.org/guide --version 3 --max-pages 500
```

### 4. 搜索文档

```bash
# 搜索索引的文档
docs-mcp-rs search react "how to use hooks"

# 在特定版本中搜索
docs-mcp-rs search vue "computed properties" --version 3
```

## CLI 命令

### `serve` - 启动 MCP 服务器

```bash
docs-mcp-rs serve [OPTIONS]

选项:
  -p, --port <PORT>  HTTP 服务端口 [默认: 26301]
      --stdio        以 stdio 模式运行
```

### `scrape` - 爬取并索引文档

```bash
docs-mcp-rs scrape <LIBRARY> <URL> [OPTIONS]

参数:
  <LIBRARY>  库名称
  <URL>      起始 URL

选项:
  -v, --version <VERSION>      版本号
  -p, --max-pages <MAX_PAGES>  最大爬取页面数 [默认: 1000]
  -d, --max-depth <MAX_DEPTH>  最大爬取深度 [默认: 3]
  -c, --concurrency <NUM>      并发数 [默认: 5]
```

### `search` - 搜索文档

```bash
docs-mcp-rs search <LIBRARY> <QUERY> [OPTIONS]

参数:
  <LIBRARY>  库名称
  <QUERY>    搜索查询

选项:
  -v, --version <VERSION>  版本号
  -l, --limit <LIMIT>      结果数量限制 [默认: 5]
```

### `list` - 列出已索引的库

```bash
docs-mcp-rs list
```

### `remove` - 删除索引

```bash
docs-mcp-rs remove <LIBRARY> [OPTIONS]

参数:
  <LIBRARY>  库名称

选项:
  -v, --version <VERSION>  版本号（不指定则删除整个库）
```

## MCP 工具

本服务器提供以下 MCP 工具供 AI 助手调用：

| 工具名 | 描述 |
|--------|------|
| `scrape_docs` | 爬取并索引文档网站 |
| `search_docs` | 搜索已索引的文档 |
| `list_libraries` | 列出所有已索引的库 |
| `remove_library` | 删除已索引的库 |

## 环境变量

| 变量名 | 描述 | 默认值 |
|--------|------|--------|
| `OPENAI_API_KEY` | OpenAI API 密钥 | - |
| `OPENAI_API_BASE` | OpenAI API 基础 URL | `https://api.openai.com/v1` |
| `GOOGLE_API_KEY` | Google API 密钥 | - |
| `GOOGLE_API_BASE` | Google API 基础 URL | - |
| `DOCS_MCP_EMBEDDING_MODEL` | 嵌入模型 (`provider:model` 格式) | `openai:text-embedding-3-small` |
| `DOCS_MCP_EMBEDDING_DELAY_MS` | 嵌入请求间隔 (毫秒) | 150 |
| `DOCS_MCP_EMBEDDING_MAX_RPM` | 每分钟最大请求数 | 1800 |
| `DOCS_MCP_EMBEDDING_MAX_TPM` | 每分钟最大 Token 数 | 800000 |
| `DOCS_MCP_EMBEDDING_MAX_RETRIES` | 429 错误最大重试次数 | 3 |
| `DOCS_MCP_EMBEDDING_RETRY_BASE_DELAY_MS` | 重试基础延迟 (毫秒) | 1000 |

## 技术架构

- **Web 框架**: Axum
- **数据库**: SQLite + sqlite-vec (向量扩展)
- **MCP 协议**: rmcp
- **嵌入 API**: async-openai, reqwest
- **HTML 解析**: scraper, fast_html2md
- **浏览器自动化**: headless_chrome

## 项目结构

```
docs-mcp-rs/
├── src/
│   ├── cli/          # 命令行参数解析
│   ├── core/         # 核心配置和类型
│   ├── embed/        # 嵌入模型集成
│   ├── events/       # 事件总线
│   ├── mcp/          # MCP 服务器实现
│   ├── pipeline/     # 爬取管道管理
│   ├── scraper/      # 网页爬取
│   ├── splitter/     # 文档分块
│   ├── store/        # 数据存储
│   └── web/          # Web 界面
├── migrations/       # 数据库迁移
├── public/           # 静态资源
└── data/             # 数据目录
```

## 相关项目

- [docs-mcp-server](https://github.com/arabold/docs-mcp-server) - 原始 TypeScript/Node.js 实现

## License

MIT License
