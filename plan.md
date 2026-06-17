# Fnine — 自托管阅读服务 计划文档

> **状态**: 已确认,待编码  
> **最后更新**: 2026-06-17

---

## 1. 项目概述

Fnine 是一个自托管的 Web 阅读服务。用户可以上传 EPUB 电子书,在浏览器中在线阅读,记录阅读进度,并添加标注/想法。

- **目标用户**: 个人单用户自托管
- **运行方式**: 单二进制文件,本地启动,浏览器访问;同时提供 Docker 镜像
- **书籍格式**: EPUB 2 / EPUB 3(通过 `rbook` 解析)
- **部署方式**: Docker 多阶段构建,使用 cargo-chef 缓存依赖层

---

## 2. 技术栈选型

| 领域 | 选型 | 理由 |
|------|------|------|
| Web 框架 | **Axum** 0.8+ | 异步,类型安全,生态成熟 |
| 模板引擎 | **Askama** (含 `askama_axum`) | ✅ 已确认。编译时模板检查,与 Axum 集成最广泛 |
| 书籍解析 | **rbook** 0.7.9 | ✅ 已确认。纯 Rust EPUB 解析,支持 EPUB 2/3 |
| 缓存 | **foyer** 0.22.3 | 混合缓存(内存+磁盘),支持 tokio 运行时 |
| 异步运行时 | **tokio** | Axum 依赖,与 foyer `runtime-tokio` feature 匹配 |
| 序列化 | **serde** + **toml** | ✅ 已确认:所有配置文件和数据文件统一使用 TOML 格式 |
| 多语言 | TOML 键值对 | 与数据层统一格式,轻量直观 |
| 静态资源 | `tower-http` `ServeDir` | Axum 生态标准 |
| 容器化 | **Docker** + **cargo-chef** | 多阶段构建,缓存依赖层加速 |

---

## 3. 持久化方案:TOML 文件 + 文件系统

### 3.1 设计原则

- 所有持久化数据统一使用 **TOML** 格式(非 JSON)
- 数据可读、可手动编辑、可版本控制
- 不做启动时全量加载,通过 foyer 缓存系统按需动态加载,冷数据从 TOML 文件透明回读
- 原子写入保证数据一致性

### 3.2 存储布局

| 数据类型 | 存储位置 | 格式 |
|----------|----------|------|
| 书籍元信息(书名、作者、封面等) | `data/books.toml` | TOML 数组 |
| 阅读进度 | `data/progress.toml` | TOML(按书籍 ID 索引) |
| 标注/想法 | `data/annotations.toml` | TOML(按书籍 ID 索引) |
| EPUB 原始文件 | `data/books/{id}.epub` | 二进制文件 |
| 用户设置(语言偏好等) | `data/settings.toml` | TOML |

**示例 — `data/books.toml`**:

```toml
[[books]]
id = "a1b2c3d4-..."
title = "深入理解计算机系统"
author = "Randal E. Bryant"
cover_path = "covers/a1b2c3d4.jpg"
chapter_count = 12
file_name = "csapp.epub"
file_size = 5242880
uploaded_at = "2026-06-17T10:30:00Z"

[[books]]
id = "e5f6g7h8-..."
title = "设计数据密集型应用"
author = "Martin Kleppmann"
cover_path = "covers/e5f6g7h8.jpg"
chapter_count = 25
file_name = "ddia.epub"
file_size = 8388608
uploaded_at = "2026-06-17T11:00:00Z"
```

**示例 — `data/settings.toml`**:

```toml
language = "zh"
theme = "sepia"
font_size = 18
```

---

## 4. 项目目录结构

```
fnine/
├── Cargo.toml
├── Cargo.lock
├── Dockerfile                     # 多阶段构建
├── .dockerignore
├── plan.md                        # 本文件
├── src/
│   ├── main.rs                    # 入口,启动服务器
│   ├── config.rs                  # 配置(数据目录、绑定地址等)
│   ├── error.rs                   # 统一错误类型
│   │
│   ├── db/                        # 数据持久化层
│   │   ├── mod.rs
│   │   ├── store.rs               # TOML 文件读写抽象
│   │   ├── books.rs               # 书籍元信息 CRUD
│   │   ├── progress.rs            # 阅读进度
│   │   └── annotations.rs         # 标注/想法
│   │
│   ├── epub/                      # EPUB 处理
│   │   ├── mod.rs
│   │   └── parser.rs              # 使用 rbook 解析 EPUB,提取元数据和内容
│   │
│   ├── cache/                     # 缓存层
│   │   └── mod.rs                 # foyer 缓存封装
│   │
│   ├── i18n/                      # 多语言
│   │   ├── mod.rs
│   │   ├── en.toml
│   │   └── zh.toml
│   │
│   ├── handlers/                  # HTTP 请求处理器
│   │   ├── mod.rs
│   │   ├── library.rs             # 书库首页、上传页
│   │   ├── reader.rs              # 在线阅读页
│   │   ├── api_books.rs           # 书籍 CRUD API
│   │   ├── api_progress.rs        # 进度更新 API
│   │   ├── api_annotations.rs     # 标注 CRUD API
│   │   └── api_settings.rs        # 设置(语言切换等)
│   │
│   └── templates/                 # Askama 模板
│       ├── base.html              # 基础布局(导航栏、页脚)
│       ├── index.html             # 书库列表页
│       ├── upload.html            # 上传页
│       ├── book_detail.html       # 书籍详情页
│       ├── reader.html            # 在线阅读器
│       └── components/            # 可复用组件
│           ├── book_card.html     # 书籍卡片
│           └── pagination.html    # 分页
│
├── static/                        # 静态资源
│   ├── css/
│   │   └── style.css
│   └── js/
│       └── reader.js              # 阅读器前端 JS(进度自动保存、标注交互)
│
└── data/                          # 运行时数据(首次启动自动创建)
    ├── books/                     # 上传的 EPUB 文件
    ├── covers/                    # 提取的封面图片
    ├── books.toml
    ├── progress.toml
    ├── annotations.toml
    └── settings.toml
```

---

## 5. Docker 部署方案

### 5.1 Dockerfile(多阶段构建 + cargo-chef)

```dockerfile
# ---- Stage 1: Planner (cargo-chef) ----
FROM rust:latest AS chef
RUN apt-get update && apt-get install -y pkg-config libssl-dev musl-tools && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo install cargo-chef
WORKDIR /app

# ---- Stage 2: Prepare recipe ----
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---- Stage 3: Build dependencies (cached) ----
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

# ---- Stage 4: Build application ----
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

# ---- Stage 5: Runtime (minimal Alpine) ----
FROM alpine:latest AS runtime
RUN apk add --no-cache ca-certificates
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/fnine /usr/local/bin/fnine
COPY --from=builder /app/static /app/static

# 数据目录通过 volume 挂载
VOLUME ["/app/data"]

EXPOSE 3000
ENV FNINE_HOST=0.0.0.0
ENV FNINE_PORT=3000
ENV FNINE_DATA_DIR=/app/data

CMD ["/usr/local/bin/fnine"]
```

### 5.2 docker-compose.yml(可选)

```yaml
services:
  fnine:
    build: .
    container_name: fnine
    ports:
      - "3000:3000"
    volumes:
      - ./data:/app/data     # 持久化数据
    environment:
      - FNINE_HOST=0.0.0.0
      - FNINE_PORT=3000
      - FNINE_DATA_DIR=/app/data
    restart: unless-stopped
```

### 5.3 构建与运行

```bash
# 构建镜像
docker build -t fnine:latest .

# 运行
docker run -d -p 3000:3000 -v $(pwd)/data:/app/data --name fnine fnine:latest
```

### 5.4 GitHub Actions CI/CD

#### 5.4.1 CI — 自动化检查与测试 (`.github/workflows/ci.yml`)

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    name: Check + Test + Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Clippy lint
        run: cargo clippy --all-targets -- -D warnings

      - name: Build
        run: cargo build --release

      - name: Test
        run: cargo test --release
```

#### 5.4.2 CD — 构建并推送 Docker 镜像 (`.github/workflows/docker.yml`)

```yaml
name: Docker

on:
  push:
    tags:
      - 'v*.*.*'

env:
  REGISTRY: ghcr.io
  IMAGE_NAME: ${{ github.repository }}

jobs:
  build-and-push:
    name: Build & Push Docker Image
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write

    steps:
      - uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to GitHub Container Registry
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract metadata (tags, labels)
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}
          tags: |
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=semver,pattern={{major}}

      - name: Build and push
        uses: docker/build-push-action@v6
        with:
          context: .
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

#### 5.4.3 使用方式

```bash
# 本地开发时,推送 tag 即可触发自动构建
git tag v0.1.0
git push origin v0.1.0

# 拉取镜像
docker pull ghcr.io/<username>/fnine:v0.1.0
```

---

## 6. 路由设计

| 方法 | 路径 | 说明 | 返回类型 |
|------|------|------|----------|
| `GET` | `/` | 书库首页(书籍列表) | HTML |
| `GET` | `/upload` | 上传页面 | HTML |
| `POST` | `/upload` | 上传 EPUB 文件 | Redirect → `/book/:id` |
| `GET` | `/book/:id` | 书籍详情页 | HTML |
| `POST` | `/book/:id/delete` | 删除书籍 | Redirect → `/` |
| `GET` | `/book/:id/read` | 在线阅读器(重定向到上次阅读章节) | HTML |
| `GET` | `/book/:id/read/:chapter` | 阅读指定章节 | HTML |
| `POST` | `/api/progress` | 更新阅读进度 (JSON) | JSON |
| `GET` | `/api/book/:id/annotations` | 获取标注列表 (JSON) | JSON |
| `POST` | `/api/book/:id/annotations` | 创建标注 (JSON) | JSON |
| `DELETE` | `/api/book/:id/annotations/:aid` | 删除标注 (JSON) | JSON |
| `GET` | `/api/book/:id/search` | 搜索书籍内容 (JSON, ?q=keyword) | JSON |
| `GET` | `/settings` | 用户设置页 | HTML |
| `POST` | `/api/settings` | 更新设置(语言等) (JSON) | JSON |
| `GET` | `/static/*` | 静态资源 | 文件内容 |

---

## 7. 数据模型

### 7.1 书籍 (Book)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Book {
    id: String,                // UUID v4
    title: String,
    author: String,
    cover_path: Option<String>, // 封面图片路径(从 EPUB 提取)
    chapter_count: u32,
    file_name: String,         // 原始文件名
    file_size: u64,
    uploaded_at: DateTime<Utc>,
}
```

### 7.2 阅读进度 (Progress)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Progress {
    book_id: String,
    chapter: u32,              // 当前章节索引(从 0 开始)
    position: f64,             // 章节内位置(0.0 ~ 1.0),用于滚动恢复
    updated_at: DateTime<Utc>,
}
```

### 7.3 标注 (Annotation)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Annotation {
    id: String,                // UUID v4
    book_id: String,
    chapter: u32,
    selected_text: String,     // 选中的文本
    note: Option<String>,      // 用户的想法/笔记(可选)
    position_start: f64,       // 标注在章节中的起始位置
    position_end: f64,         // 标注在章节中的结束位置
    color: String,             // 高亮颜色
    created_at: DateTime<Utc>,
}
```

### 7.4 设置 (Settings)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    language: String,          // "en" | "zh"
    theme: String,             // "light" | "dark" | "sepia"
    font_size: u32,            // 阅读字体大小(px)
}
```

---

## 8. 核心模块说明

### 8.1 EPUB 解析 (`epub/parser.rs`)

使用 `rbook` 完成:

1. **元数据提取**: 书名、作者、封面图片
2. **目录解析**: 提取 spine/toc 生成章节列表
3. **内容提取**: 按章节提取 HTML/XHTML,内嵌到阅读器页面
4. **封面提取**: 解析 cover image,保存为独立文件供列表页展示
5. **全文搜索**: 遍历章节内容建立内存倒排索引

### 8.2 缓存 (`cache/mod.rs`)

使用 `foyer` 缓存:

| 缓存键 | 缓存值 | 策略 |
|--------|--------|------|
| `book:{id}` | Book 元数据 | LRU 容量淘汰 |
| `chapter:{book_id}:{chapter}` | 章节 HTML 内容 | LRU(频繁访问保留) |
| `search:{query_hash}` | 搜索结果 | TTL 5 分钟 |
| `cover:{id}` | 封面图片 bytes | LRU |

主要使用 foyer 内存缓存能力,减轻 TOML 文件频繁读取。

### 8.3 多语言 (`i18n/`)

TOML 格式,与数据层统一:

```toml
# zh.toml
[ui]
nav_home = "书库"
nav_upload = "上传"
nav_settings = "设置"

[book]
progress = "阅读进度"
no_books = "还没有书籍,去上传一本吧"

[reader]
prev_chapter = "上一章"
next_chapter = "下一章"
toc = "目录"

[annotation]
add = "添加标注"
delete = "删除"
note_placeholder = "写下你的想法..."

# en.toml
[ui]
nav_home = "Library"
nav_upload = "Upload"
nav_settings = "Settings"

[book]
progress = "Reading Progress"
no_books = "No books yet. Upload one!"

[reader]
prev_chapter = "Previous"
next_chapter = "Next"
toc = "Table of Contents"

[annotation]
add = "Add Annotation"
delete = "Delete"
note_placeholder = "Write your thoughts..."
```

语言检测优先级:

1. Cookie / `data/settings.toml` 中持久化的偏好
2. 浏览器 `Accept-Language` 头
3. 默认回退 `en`

### 8.4 在线阅读器 (`handlers/reader.rs` + `templates/reader.html`)

核心体验模块:

1. **章节渲染**: rbook 提取的章节 HTML 嵌入 `reader.html` 模板
2. **滚动模式**: 全文加载,恢复上次滚动位置(通过 `position` 字段)
3. **进度自动保存**: JS 定期(滚动停止 2 秒后)通过 `/api/progress` 记录
4. **标注交互**:
   - 用户选中文字 → 弹出标注工具栏
   - 保存标注到后端
   - 重新打开时渲染已有高亮
5. **目录侧栏**: 左侧显示章节目录,点击跳转
6. **主题切换**: light / dark / sepia,CSS 变量实现
7. **键盘导航**: ← → 切换章节(可选)

---

## 9. Cargo.toml 依赖清单

```toml
[package]
name = "fnine"
version = "0.1.0"
edition = "2024"

[dependencies]
# Web 框架
axum = "0.8"
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.6", features = ["fs", "limit", "trace"] }
tower = "0.5"

# 模板
askama = { version = "0.14", features = ["with-axum"] }
askama_axum = "0.5"

# EPUB 解析
rbook = "0.7"

# 缓存
foyer = { version = "0.22", features = ["runtime-tokio"] }

# 序列化(TOML + JSON, API 返回 JSON)
serde = { version = "1", features = ["derive"] }
toml = "0.8"
serde_json = "1.0"

# 工具
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "2"
```

> 注:JSON 已替换为 `toml`,API 接口中仍使用 JSON 作为请求/响应格式(前端 JS 原生支持)。

---

## 10. 实施计划

### 阶段 1: 项目骨架 ✅

- [x] 初始化 `Cargo.toml`,添加依赖
- [x] 编写 `Dockerfile`(多阶段构建 + cargo-chef)
- [x] 编写 `.dockerignore`
- [x] 实现 `config.rs` — 读取环境变量(`FNINE_HOST`, `FNINE_PORT`, `FNINE_DATA_DIR`)
- [x] 实现 `error.rs` — 统一错误类型和 `IntoResponse`
- [x] 搭建 `main.rs` — Axum 路由注册 + 启动,嵌入 `tower-http` 中间件
- [x] 创建 Askama 基础模板(`base.html`)
- [x] 实现 TOML 文件存储层(`db/store.rs`)
- [x] GitHub Actions CI/CD workflows

### 阶段 2: 书籍管理 ✅

- [x] 实现 EPUB 上传(`POST /upload`,multipart 表单)
- [x] 使用 rbook 解析 EPUB 元数据
- [x] 实现书籍列表(`GET /`) + 书籍卡片组件
- [x] 实现书籍详情(`GET /book/:id`)
- [x] 实现书籍删除(文件 + TOML 记录)
- [x] 实现封面提取和显示

### 阶段 3: 在线阅读 ✅

- [x] 实现章节内容提取(EPUB spine 解析)
- [x] 实现阅读器页面(`GET /book/:id/read/:chapter`)
- [x] 实现章节目录侧栏
- [x] 实现主题切换 CSS(light/dark/sepia)

### 阶段 4: 进度与标注

- [ ] 实现阅读进度 API(`POST /api/progress`)
- [ ] 实现前端进度自动保存(JS)
- [ ] 实现标注 CRUD API
- [ ] 实现前端文本选中 + 标注工具栏
- [ ] 实现标注高亮渲染

### 阶段 5: 多语言

- [ ] 实现 i18n 模块(`i18n/mod.rs`,加载 TOML 翻译文件)
- [ ] 集成到 Askama 模板中
- [ ] 实现语言切换
- [ ] 完善中英文翻译

### 阶段 6: 缓存与优化

- [ ] 集成 foyer 缓存(章节内容、书籍元数据)
- [ ] 添加全文搜索功能
- [ ] 性能调优
- [ ] 错误处理完善

---

## 11. 环境变量配置

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `FNINE_HOST` | `127.0.0.1` | 监听地址 |
| `FNINE_PORT` | `3000` | 监听端口 |
| `FNINE_DATA_DIR` | `./data` | 数据目录路径 |
| `RUST_LOG` | `info` | 日志级别(tracing) |

Docker 镜像中将 `FNINE_HOST` 设为 `0.0.0.0`。

---

## 12. 最终确认清单

| 项目 | 决策 |
|------|------|
| 模板引擎 | ✅ Askama |
| 持久化格式 | ✅ TOML(所有配置和数据文件) |
| 用户模型 | ✅ 单用户 |
| 书籍格式 | ✅ 仅 EPUB(rbook 解析) |
| 部署方式 | ✅ Docker 多阶段构建 + cargo-chef |
| 前端 JS | ✅ 少量原生 JS(进度保存、标注交互) |
| 多语言 | ✅ TOML 格式翻译文件 |

---

> **状态**: 计划已确认。等待你最终的 "开始编码" 指令。
