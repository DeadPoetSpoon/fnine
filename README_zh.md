# Fnine

一个使用 Rust 编写的自托管 EPUB 阅读服务。在浏览器中上传、管理和阅读你的电子书——无需数据库。

## 功能特性

- **EPUB 上传** — 支持 EPUB 2 和 EPUB 3 格式。自动提取元数据（书名、作者、封面）。
- **在线阅读器** — 简洁的逐章阅读体验，内置目录侧边栏。
- **阅读进度** — 自动保存滚动位置，随时从上次中断处继续阅读。
- **标注与笔记** — 高亮文本、选择颜色，为任意段落添加个人笔记。
- **搜索** — 按书名或作者即时查找书籍。
- **多语言** — 提供英文和中文界面。可轻松扩展更多语言。
- **主题切换** — 支持浅色和深色阅读主题。
- **自定义字体** — 可为阅读器上传 `.ttf` 或 `.woff2` 字体文件。
- **无需数据库** — 所有数据以纯文本 TOML 文件持久化存储。零配置，易于备份。
- **内存缓存** — 章节内容和书籍列表缓存在内存中，响应快速。
- **Docker 就绪** — 基于 `cargo-chef` 的多阶段 Dockerfile，构建高效。最终镜像基于 Alpine，体积小巧。

## 截图

*截图即将添加。*

## 快速开始

### 使用 Docker

```bash
docker run -d \
  --name fnine \
  -p 3000:3000 \
  -v fnine-data:/app/data \
  ghcr.io/deadpoetspoon/fnine:latest
```

### 从源码构建

**前置条件：** Rust 1.96+（edition 2024）。

```bash
git clone https://github.com/DeadPoetSpoon/fnine.git
cd fnine
cargo run --release
```

服务器将在 `http://0.0.0.0:3000` 启动。

## 配置

Fnine 通过环境变量进行配置：

| 变量            | 默认值      | 说明             |
| --------------- | ---------- | ---------------- |
| `FNINE_HOST`    | `0.0.0.0`  | 绑定的 IP 地址    |
| `FNINE_PORT`    | `3000`     | 监听端口          |
| `FNINE_DATA_DIR`| `./data`   | 持久化数据目录    |

## 项目结构

```
fnine/
├── src/
│   ├── main.rs           # 入口点，路由配置
│   ├── config.rs         # 环境变量配置
│   ├── state.rs          # 共享应用状态
│   ├── error.rs          # 统一错误类型
│   ├── cache/
│   │   └── mod.rs        # 内存缓存
│   ├── db/
│   │   ├── mod.rs
│   │   ├── store.rs      # 基于 TOML 的通用持久化存储
│   │   ├── books.rs      # 书籍数据模型
│   │   ├── progress.rs   # 阅读进度数据模型
│   │   ├── annotations.rs# 标注数据模型
│   │   └── settings.rs   # 用户设置数据模型
│   ├── epub/
│   │   ├── mod.rs
│   │   └── parser.rs     # EPUB 元数据与章节提取
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── library.rs    # 首页、上传表单、书籍详情、封面图片
│   │   ├── reader.rs     # 带章节导航的阅读器页面
│   │   ├── search.rs     # 书籍搜索
│   │   ├── api_books.rs  # 上传/删除书籍 API
│   │   ├── api_progress.rs  # 保存阅读进度 API
│   │   ├── api_annotations.rs# 标注增删查 API
│   │   └── api_settings.rs  # 设置页面与字体上传
│   └── i18n/
│       ├── mod.rs
│       ├── translations.rs  # 翻译加载与扁平化
│       ├── en.toml          # 英文翻译
│       └── zh.toml          # 中文翻译
├── templates/
│   ├── base.html            # 带导航栏的基础布局
│   ├── index.html           # 书库首页（书籍网格）
│   ├── upload.html          # 上传表单
│   ├── book_detail.html     # 书籍详情与标注
│   ├── reader.html          # 在线阅读器
│   ├── search.html          # 搜索结果
│   ├── settings.html        # 设置页面
│   └── components/
│       └── book_card.html   # 可复用的书籍卡片组件
├── static/
│   ├── css/                 # 样式表
│   └── js/                  # 客户端 JavaScript
├── data/                    # 默认数据目录（Docker 中挂载为卷）
│   ├── books/               # 存储的 EPUB 文件
│   ├── covers/              # 提取的封面图片
│   ├── fonts/               # 用户上传的字体
│   ├── annotations/         # 每本书的标注 TOML 文件
│   ├── books.toml           # 书籍元数据索引
│   ├── progress.toml        # 每本书的阅读进度
│   ├── settings.toml        # 用户设置
│   └── annotations.toml     # （保留）
├── Dockerfile               # 多阶段 Docker 构建
├── Cargo.toml               # Rust 依赖
└── .github/workflows/       # CI/CD 流水线
    ├── ci.yml               # 格式化、检查、构建、测试
    └── docker.yml            # 构建并推送 Docker 镜像
```

## 技术栈

| 组件         | 依赖库 / 技术                  |
| ------------ | ----------------------------- |
| Web 框架     | [axum](https://crates.io/crates/axum) 0.8 |
| 模板引擎     | [askama](https://crates.io/crates/askama) 0.16 |
| EPUB 解析    | [rbook](https://crates.io/crates/rbook) 0.7 |
| 异步运行时   | [tokio](https://crates.io/crates/tokio) 1.52 |
| 序列化       | [serde](https://crates.io/crates/serde) + [toml](https://crates.io/crates/toml) |
| 中间件       | [tower-http](https://crates.io/crates/tower-http) 0.7 |
| 日志         | [tracing](https://crates.io/crates/tracing) 0.1 |
| ID 生成      | [uuid](https://crates.io/crates/uuid) 1.23 (v4) |
| 时间戳       | [chrono](https://crates.io/crates/chrono) 0.4 |

## API 概览

| 方法   | 路径                                  | 说明                 |
| ------ | ------------------------------------- | -------------------- |
| `GET`  | `/`                                   | 书库首页             |
| `GET`  | `/upload`                             | 上传表单             |
| `POST` | `/upload`                             | 上传 EPUB 文件        |
| `GET`  | `/book/{id}`                          | 书籍详情页           |
| `POST` | `/book/{id}/delete`                   | 删除书籍             |
| `GET`  | `/book/{id}/read`                     | 跳转到上次阅读章节    |
| `GET`  | `/book/{id}/read/{chapter}`           | 阅读指定章节         |
| `GET`  | `/covers/{id}`                        | 提供封面图片         |
| `GET`  | `/search?q=`                          | 搜索书籍             |
| `GET`  | `/settings`                           | 设置页面             |
| `POST` | `/settings`                           | 保存设置             |
| `POST` | `/settings/fonts`                     | 上传字体文件         |
| `POST` | `/settings/fonts/delete`              | 删除字体文件         |
| `POST` | `/api/progress`                       | 保存阅读进度         |
| `GET`  | `/api/book/{id}/annotations`          | 列出标注             |
| `POST` | `/api/book/{id}/annotations`          | 创建标注             |
| `POST` | `/api/book/{id}/annotations/{aid}`    | 删除标注             |

## 许可证

MIT
