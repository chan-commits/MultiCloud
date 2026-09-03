# MultiCloud

MultiCloud 是以 Rust 构建的多租户云端管理控制平台。平台以 Organization 作为租户边界，统一管理云端 Provider、VPS/DNS Asset、Ticket、Live Chat、Audit、Billing 与部署于 VPS 的 Rust Agent。

## 技术栈

- Backend：Rust、Axum、Tokio、SeaORM、PostgreSQL、Redis、WebSocket
- Frontend：SvelteKit、Svelte 5、TypeScript、Vite、TailwindCSS
- Tooling：`just`、Podman/Docker Compose、GitHub Actions

Web UI 支持 English、简体中文和繁體中文。首次访问按浏览器系统语言选择中文变体，未匹配时回退 English；用户手动选择后会保存在浏览器本地。

## 架构原则

- Domain Driven Design 与 bounded context
- Modular Monolith 起步，保留拆分服务的边界
- PostgreSQL 是业务资料唯一真实来源
- `organization_id` repository scope 与 PostgreSQL RLS 双重租户隔离
- 外部操作采用 Operation、transactional outbox、幂等与重试
- Provider 采用 capability-oriented adapter abstraction
- Domain 不依赖 Axum、SeaORM、Redis 或 Provider SDK

## Workspace 與單一執行檔

```text
apps/                 單一 multicloud 執行檔與可重用 runtime crates
crates/               Domain 与 Infrastructure crates
migrations/           SeaORM migrations
frontend/web/         SvelteKit static SPA（編譯後嵌入 Rust binary）
config/               分层配置
doc/                  需求、架构、Schema、流程与开发路线
```

## 本地开发

先复制环境变量并启动基础设施：

```bash
cp .env.example .env
just infra-up
just admin-init
```

安装依赖及启动应用：

```bash
just bootstrap
just run
just dev-web
```

Podman Compose、Docker Compose 兼容方式，以及不使用容器的 PostgreSQL/Redis 本机部署、建库、migration 和首次初始化步骤见 [部署指南](doc/deployment.md)。应用不会使用 PostgreSQL 超级用户自动建库；系统管理员只需建立 role 与空 database。首次执行 `multicloud init` 会先应用 pending migrations，再交互建立管理员；后续升级可单独执行 `multicloud migrate up`。

生产部署只需 `multicloud` 一个可执行文件；前端 `dist` 会在编译时嵌入 binary，运行时不需要独立的静态文件服务器。默认 `serve` 会在同一进程启动 API、Worker 与 Scheduler。`worker`、`scheduler` 和 `agent` 子命令用于隔离调试或特殊部署。首次安装通过服务器上的交互式管理命令建立首位管理员及 Organization；密码不会出现在 shell history 或 process list。若管理员无法登入，可在服务器终端执行：

```bash
just recover-access
# 或指定用户；多 Organization 时会要求选择目标租户
just recover-access admin@example.com
```

公开注册默认关闭。首位 Platform Admin 初始化并登录后，可从 Web 顶部控制栏开启；开启期间普通使用者可在登录页注册账号，并在首次登入后建立自己的 Organization。Organization Owner/Admin 无权修改平台注册策略。

Platform Admin 也可在 Web 顶栏动态调整 application log level；设置持久化到数据库并立即生效。日志正文仍写往 stdout/journald，磁盘容量由宿主机或容器日志后端控制，详见部署指南。

执行完整检查：

```bash
just check
```

建立 Debug 产物（包含前置依赖安装、格式/静态检查、测试、前端构建与单一 Rust binary）：

```bash
just build
```

生产 Release 构建：

```bash
./build.sh --release
```

产物分别位于 `target/debug/multicloud` 或 `target/release/multicloud`，前端产物位于 `frontend/web/dist/`。

`main` 分支通过 CI 后会保存 30 天的 Release 单二进制 Artifact，以 commit SHA 命名。打开对应的 GitHub Actions CI run，可从顶部 Summary 的 **Download** 链接或页面底部 **Artifacts** 区域下载。正式版本使用语义化 `v*` tag 发布长期保存的 GitHub Release，并附带 Linux x86_64 压缩包与 SHA-256 校验文件：

```bash
git tag -a v0.1.0 -m "v0.1.0"
git push origin v0.1.0
```

所有常用命令可通过 `just --list` 查看。

## 当前进度

- Phase 0：工程基线已完成
- Phase 1：Identity、Organization、TenantContext 与 RLS 已完成
- Phase 2：Organization-scoped RBAC 已完成
- Phase 3：Operation、transactional outbox、inbox idempotency 与 Worker retry 已完成
- Phase 4：Provider abstraction、加密 credential、Fake 与 Cloudflare adapter 已完成
- Phase 5：Resource Management 与 Real Provider Integration 已完成（Provider backend、Resource/Operation/Drift 与 Command Center UI）
- Phase 6：append-only Audit Log、递归脱敏、查询/CSV 导出、分区/retention policy 与 Audit Stream UI 已完成
- Phase 7：Ticket lifecycle、comment、attachment metadata、assignment、SLA、notification、RBAC 与 Support Desk UI 已完成
- 后续阶段：Live Chat、Agent、Billing

详细规划见 [开发顺序](doc/development-roadmap.md)，系统设计入口见 [设计文件索引](doc/README.md)。

## 设计文件

- [需求基线](doc/requirements.md)
- [系统架构](doc/architecture.md)
- [Module 设计](doc/modules.md)
- [Database Schema](doc/database-schema.md)
- [核心流程](doc/workflows.md)

## License

MIT
