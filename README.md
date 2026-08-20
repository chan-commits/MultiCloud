# MultiCloud

MultiCloud 是以 Rust 构建的多租户云端管理控制平台。平台以 Organization 作为租户边界，统一管理云端 Provider、VPS/DNS Asset、Ticket、Live Chat、Audit、Billing 与部署于 VPS 的 Rust Agent。

## 技术栈

- Backend：Rust、Axum、Tokio、SeaORM、PostgreSQL、Redis、WebSocket
- Frontend：Svelte、TypeScript、Vite、TailwindCSS
- Tooling：`just`、Podman/Docker Compose、GitHub Actions

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
frontend/web/         Svelte Web Application
config/               分层配置
doc/                  需求、架构、Schema、流程与开发路线
```

## 本地开发

先复制环境变量并启动基础设施：

```bash
cp .env.example .env
just infra-up
just migrate up
just admin-init
```

安装依赖及启动应用：

```bash
just bootstrap
just run
just dev-web
```

生产部署只需 `multicloud` 一个可执行文件；默认 `serve` 会在同一进程启动 API、Worker 与 Scheduler。`worker`、`scheduler` 和 `agent` 子命令用于隔离调试或特殊部署。首次安装通过服务器上的交互式管理命令建立首位管理员及 Organization；密码不会出现在 shell history 或 process list。若管理员无法登入，可在服务器终端执行：

```bash
just recover-access
# 或指定用户；多 Organization 时会要求选择目标租户
just recover-access admin@example.com
```

公开注册默认关闭。首位 Platform Admin 初始化并登录后，可从 Web 顶部控制栏开启；开启期间普通使用者可在登录页注册账号，并在首次登入后建立自己的 Organization。Organization Owner/Admin 无权修改平台注册策略。

执行完整检查：

```bash
just check
```

所有常用命令可通过 `just --list` 查看。

## 当前进度

- Phase 0：工程基线已完成
- Phase 1：Identity、Organization、TenantContext 与 RLS 已完成
- Phase 2：Organization-scoped RBAC 已完成
- Phase 3：Operation、transactional outbox、inbox idempotency 与 Worker retry 已完成
- Phase 4：Provider abstraction、加密 credential、Fake 与 Cloudflare adapter 已完成
- Phase 5：Resource Management 与 Real Provider Integration 已完成（Provider backend、Resource/Operation/Drift 与 Command Center UI）
- Phase 6：append-only Audit Log、递归脱敏、查询/CSV 导出、分区/retention 基础与 Audit Stream UI 已完成
- 后续阶段：Audit、Ticket/Chat、Agent、Billing

详细规划见 [开发顺序](doc/development-roadmap.md)，系统设计入口见 [设计文件索引](doc/README.md)。

## 设计文件

- [需求基线](doc/requirements.md)
- [系统架构](doc/architecture.md)
- [Module 设计](doc/modules.md)
- [Database Schema](doc/database-schema.md)
- [核心流程](doc/workflows.md)

## License

MIT
