# Fnine

一个使用 Rust 编写的自托管 EPUB 阅读服务。在浏览器中上传、管理和阅读你的电子书——无需数据库。

## 功能特性

- **EPUB 上传** — 支持 EPUB 2 和 EPUB 3 格式。自动提取元数据（书名、作者、封面）。
- **在线阅读器** — 简洁的逐章阅读体验，内置目录侧边栏。无标题的分段沿用前一章名称，并按顺序显示本地化的续篇编号。
- **脚注与尾注** — 紧贴前文右上方的上标引用，点击即可弹出注释正文，支持独立 EPUB 注释文档、CSS 上标、无链接的分组圆圈数字注释及键盘操作。
- **阅读进度** — 自动保存滚动位置，随时从上次中断处继续阅读。
- **标注与笔记** — 高亮文本、选择颜色，为任意段落添加个人笔记。
- **书库管理** — 继续阅读、书名/作者/上传/最近阅读排序及未读、在读、读完筛选。
- **笔记编辑与导出** — 修改笔记和颜色，并导出 Markdown。
- **多语言** — 提供英文和中文界面。可轻松扩展更多语言。
- **主题切换** — 支持浅色、深色和纸张主题，保存的语言和主题自动生效。
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
  -p 127.0.0.1:3000:3000 \
  -v fnine-data:/app/data \
  ghcr.io/deadpoetspoon/fnine:latest
```

### 从源码构建

**前置条件：** Rust 1.96+（edition 2024）。

```bash
git clone https://github.com/DeadPoetSpoon/fnine.git
cd fnine
cargo run --locked --release
```

服务器将在 `http://127.0.0.1:3000` 启动。请在仓库根目录运行，以便提供 `static/` 资源。

## 配置

Fnine 通过环境变量进行配置：

| 变量            | 默认值      | 说明             |
| --------------- | ---------- | ---------------- |
| `FNINE_HOST`    | `127.0.0.1`  | 绑定的 IP 地址（容器内部默认 `0.0.0.0`） |
| `FNINE_PORT`    | `3000`     | 监听端口          |
| `FNINE_DATA_DIR`| `./data`   | 持久化数据目录    |

## 阅读与存储边界

这是**文字优先**的阅读器：保留段落、实体、Unicode、目录标题及注释正文；不复现原书 CSS、插图、公式或复杂注释排版。失效注释会明确提示不可用，不会联网读取注释或直接渲染上传的 HTML。

每个数据目录仅允许**一个进程**使用。请求协调 TOML 修改与缓存访问，使用独立临时文件、同步和原子替换保存。导入/删除日志会在启动时恢复，但这不等同于分布式数据库或跨文件事务快照。

旧书首次阅读时升级文本坐标；原始元数据、进度和笔记备份到 `data/backups/`。仅重新定位唯一匹配的标注，模糊记录保留并提示复核。无法安全映射的进度从该章开头恢复。旧字体文件迁移到使用生成 ID 的字体清单。

限制：EPUB 请求 50 MiB、字体文件 10 MiB（另有受限表单开销）、JSON/表单 256 KiB、单条选文/笔记 16 KiB、解压后单资源 16 MiB、合计 256 MiB、最多 10,000 个归档项。章节缓存 64 MiB、目录缓存 4 MiB，解析最多两个阻塞任务。单条注释正文最多 64 KiB，每章合计最多 4 MiB。

## 私有部署与备份

服务**没有身份验证**，能够连接的人都能读取、上传、修改和删除数据。请保留回环地址发布，或使用可信私网/带认证的反向代理。源码局域网访问需要显式设置 `FNINE_HOST=0.0.0.0`；Docker 需要有意调整主机端口绑定。

容器**默认以 root 运行**，兼容已有数据挂载；升级无需将所有权改为 UID `10001`。如需选择非 root 模式，在 `docker run` 的镜像名称之前添加 `--user fnine:fnine`。该账户的 UID/GID 为 `10001`；切换前请停止 Fnine，并确保整个挂载数据目录允许该用户写入。非 root 模式可降低容器进程权限，但不是默认模式。

浏览器修改请求的 Origin 必须匹配主机和协议；跨站 Fetch Metadata 请求被拒绝。无 Origin 的命令行客户端仍可使用。可信 HTTPS 代理应保留 Host，并设置 `X-Forwarded-Proto: https`。这些检查不能代替认证。

备份步骤：停止服务，复制**整个**数据目录（EPUB、字体、TOML、日志及升级备份），再启动。恢复步骤：停止服务，恢复完整副本，校正权限，仅启动一个实例。不要在运行中手改 TOML，也不要把在线复制当作一致快照。数据目录必须是可信本地存储，不得允许不可信用户写入。

## 验证

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
npm ci
npx playwright install chromium
npm run test:browser
```

浏览器测试优先使用已安装的 `/usr/bin/chromium`，可通过 `FNINE_BROWSER` 指定其他路径。字体加载测试使用系统中的 DejaVu、Liberation 或 iA Writer TTF；也可通过 `FNINE_TEST_FONT` 指定有效 TTF 路径。Node/Playwright 仅用于开发测试，不是服务运行依赖。EPUB 2/3 合成样例覆盖数字双向链接、CSS 上标、局部重复的圆圈数字，以及通过列表显式编号纠正印错的注释标记。无法可靠匹配或缺失的正文会显示不可用，不会猜测内容。

可对含注释的本地 EPUB 执行只读检查，并在隔离的 Chromium 测试书库中验证弹窗：

```bash
FNINE_TEST_BOOKS=data/books cargo test --locked --offline local_epub_endnote_regression -- --ignored --nocapture
FNINE_TEST_BOOKS=data/books npm run test:browser
```

检查会遍历每本书的全部 spine 章节，验证引用均有正文且章节文本与坐标不变；目录中的每本 EPUB 都应含有至少一条可识别注释。浏览器测试会打开每本书中的一个真实注释弹窗，不改动原始文件或书库记录。

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
│   ├── fonts.toml           # 字体 ID 与显示名清单
│   ├── staging/             # 上传暂存
│   ├── imports/             # 导入恢复日志
│   ├── deletions/           # 删除恢复日志
│   ├── backups/             # 旧坐标升级备份
│   ├── annotations/         # 每本书的标注 TOML 文件
│   ├── books.toml           # 书籍元数据索引
│   ├── progress.toml        # 每本书的阅读进度
│   └── settings.toml        # 用户设置
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
| `GET`  | `/book/{id}/download`                 | 下载原始 EPUB 文件    |
| `POST` | `/book/{id}/delete`                   | 删除书籍             |
| `GET`  | `/book/{id}/read`                     | 跳转到上次阅读章节    |
| `GET`  | `/book/{id}/read/{chapter}`           | 阅读指定章节         |
| `GET`  | `/covers/{id}`                        | 提供封面图片         |
| `GET`  | `/settings`                           | 设置页面             |
| `POST` | `/settings`                           | 保存设置             |
| `POST` | `/settings/fonts`                     | 上传字体文件         |
| `POST` | `/settings/fonts/delete`              | 删除字体文件         |
| `POST` | `/api/progress`                       | 保存阅读进度         |
| `GET`  | `/api/recent`                          | 最近阅读的书籍 (JSON) |
| `GET`  | `/api/book/{id}/annotations`          | 列出标注             |
| `POST` | `/api/book/{id}/annotations`          | 创建标注             |
| `POST` | `/api/book/{id}/annotations/{aid}`    | 删除标注             |
| `PATCH` | `/api/book/{id}/annotations/{aid}`   | 编辑笔记和颜色        |
| `GET` | `/book/{id}/annotations/export`       | 下载 Markdown 标注    |

## 许可证

MIT
