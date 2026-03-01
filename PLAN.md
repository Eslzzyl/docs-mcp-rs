# docs-mcp-server Rust 复刻计划

## 技术选型

| 模块 | TypeScript | Rust 替代方案 |
|------|-----------|--------------|
| 异步运行时 | Node.js | `tokio` |
| MCP 协议 | @modelcontextprotocol/sdk | `rmcp` (官方 SDK) |
| HTTP 服务 | Fastify | `axum` (含 rmcp-actix-web 集成) |
| 数据库 | better-sqlite3 | `rusqlite` + `rusqlite-pool` |
| 向量扩展 | sqlite-vec | `sqlite-vec` |
| HTTP 客户端 | axios | `reqwest` + `reqwest-retry` |
| HTML 解析 | cheerio, jsdom | `scraper` |
| Markdown | remark, turndown | `pulldown-cmark`, `html2md` |
| CLI | yargs | `clap` |
| 序列化 | zod, JSON | `serde`, `serde_json` |
| 日志 | winston/pino | `tracing` + `tracing-subscriber` |
| 错误处理 | 自定义 | `thiserror`, `anyhow` |
| 配置 | dotenv, yaml | `dotenvy`, `config` |

## 项目结构

```
docs-mcp-rs/
├── Cargo.toml                    # Workspace 配置
├── crates/
│   ├── docs-mcp/                 # 主程序 (CLI + MCP Server)
│   ├── docs-core/                # 核心类型和工具
│   ├── docs-store/               # 数据存储层
│   ├── docs-scraper/             # 内容抓取
│   ├── docs-splitter/            # 文档分割
│   ├── docs-embed/               # Embedding API 客户端
│   ├── docs-pipeline/            # 异步作业处理
│   └── docs-api/                 # REST API (前后端分离)
├── migrations/                   # 数据库迁移
└── web/                          # 前端项目 (可选，后续)
```

## 分阶段实施

### 阶段 1: 基础设施 (Foundation)
**目标：搭建项目骨架和核心基础设施**

1. **项目初始化**
   - 创建 Cargo workspace 结构
   - 配置依赖和特性开关（使用 cargo add）

2. **核心模块 (docs-core)**
   - 错误类型定义 (`thiserror`)
   - 配置系统 (`config` + `dotenvy`)
   - 日志系统 (`tracing`)
   - 核心领域类型 (Library, Version, Page, Document)

3. **数据库层 (docs-store)**
   - SQLite 连接池
   - 数据库迁移系统
   - 基础 CRUD 操作
   - FTS5 全文搜索支持

**交付物：** 可编译的项目骨架 + 基础数据库操作

---

### 阶段 2: 内容处理 (Content Processing)
**目标：实现文档抓取和分割**

1. **抓取模块 (docs-scraper)**
   - HTTP 客户端 (reqwest + 重试)
   - 内容获取器 (Fetcher)
   - HTML 解析和清洗
   - HTML → Markdown 转换
   - 链接提取和爬取策略
   - robots.txt 解析

2. **分割模块 (docs-splitter)**
   - Markdown 分割器
   - 代码分割器 (基于缩进/语法)
   - 递归字符分割器
   - 分块元数据管理

**交付物：** 可抓取网站并分割文档的模块

---

### 阶段 3: 向量搜索 (Vector Search)
**目标：实现向量嵌入和混合搜索**

1. **Embedding 模块 (docs-embed)**
   - OpenAI API 集成
   - Google Gemini API 集成
   - 统一 Embedding trait
   - 批量嵌入支持

2. **向量存储扩展**
   - 集成 sqlite-vec 扩展：https://alexgarcia.xyz/sqlite-vec/rust.html
   - 向量索引和查询
   - 混合搜索 (向量 + FTS)
   - RRF (Reciprocal Rank Fusion) 排序

3. **内容组装**
   - Markdown 组装策略
   - 层级组装策略
   - 上下文扩展

**交付物：** 完整的向量搜索功能

---

### 阶段 4: MCP 服务器 (MCP Server)
**目标：实现 MCP 协议和工具**

1. **MCP 核心 (docs-mcp)**
   - 集成 `rmcp` SDK
   - Tool 定义和注册
   - stdio 传输
   - HTTP SSE 传输 (可选)

2. **工具实现**
   - `scrape_docs` - 抓取文档
   - `search_docs` - 搜索文档
   - `list_libraries` - 列出库
   - `remove_library` - 删除库

**交付物：** 可用的 MCP 服务器

---

### 阶段 5: 异步管道 (Pipeline)
**目标：实现后台作业处理**

1. **Pipeline 模块 (docs-pipeline)**
   - 作业队列 (内存或 SQLite)
   - 进度追踪
   - 错误处理和重试
   - 并发控制

2. **事件系统**
   - 简单事件总线
   - 进度通知

**交付物：** 可处理长时间运行的抓取任务

---

### 阶段 6: CLI 和 API (Interfaces)
**目标：提供用户接口**

1. **CLI (docs-mcp)**
   - `clap` 命令定义
   - 子命令: serve, scrape, search, list, remove
   - 配置文件支持

2. **REST API (docs-api)**
   - `axum` HTTP 服务器
   - OpenAPI 文档 (utoipa)
   - CORS 支持
   - 认证中间件 (可选)

**交付物：** 完整的 CLI 工具和 REST API

---

### 阶段 7: 优化和完善 (Polish)
**目标：生产就绪**

1. **性能优化**
   - 并发调优
   - 内存优化
   - 数据库索引优化

2. **文档和测试**
   - 单元测试
   - 集成测试
   - README 和使用文档

3. **部署支持**
   - Docker 镜像
   - 发布构建

**交付物：** 生产就绪的完整实现
