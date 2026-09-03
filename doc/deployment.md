# Deployment Guide

MultiCloud 发布为单个 Linux binary，SvelteKit 前端已嵌入其中；PostgreSQL 和 Redis 是外部运行时服务。保存任何 Provider credential 前，必须生成独立的 credential master key。

## Podman Compose（推荐）

安装 Podman 与 Compose provider（发行版包名可能不同），在仓库根目录执行：

```bash
cp .env.example .env
openssl rand -base64 32
# 将结果写入 .env 的 MULTICLOUD__PROVIDER__CREDENTIAL_MASTER_KEY
podman compose up -d
podman compose ps
podman compose logs postgres redis
just admin-init
just run
```

默认映射 PostgreSQL `localhost:55432`、Redis `localhost:56379`、Web/API `http://localhost:8080`。确认服务：

```bash
podman compose exec postgres pg_isready -U multicloud -d multicloud
podman compose exec redis redis-cli ping
curl --fail http://localhost:8080/health
```

`compose.yaml` 使用标准 Compose 结构并显式引用 Docker Hub 镜像，兼容 Docker Compose。Docker 用户将 `podman compose` 换成 `docker compose` 即可；`just infra-up/down` 默认使用 Podman。Named volumes 保存持久数据，`compose down` 不会删除数据；不要在生产环境执行 `compose down -v`。

首次创建 volume 时，初始化脚本会建立无 superuser、无 `CREATEDB`/`CREATEROLE` 权限的 `multicloud` 应用角色。官方 PostgreSQL 镜像中的 `POSTGRES_USER` 属于数据库管理员，应用不能直接使用它，否则会绕过 RLS。旧的开发 volume 若曾用 `multicloud` 作为 `POSTGRES_USER`，需在确认不保留本地数据后重建 volume，或由管理员手动执行 `ALTER ROLE multicloud NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT`。

## 本机服务部署（不使用容器）

安装 PostgreSQL 17、Redis 8（较新的兼容版本亦可），并将下载的 `multicloud` 放在 `/usr/local/bin/`。以 PostgreSQL 管理员执行一次：

```sql
CREATE ROLE multicloud LOGIN PASSWORD 'replace-with-a-strong-password';
CREATE DATABASE multicloud OWNER multicloud;
```

由系统管理员创建 role/database、由应用 migration 管理 schema 是合理的权限边界。应用不应取得 PostgreSQL superuser 权限；migration 会建立 table、index、RLS、partition 与 trigger。

建立仅 root 可读的 `/etc/multicloud.env`：

```bash
MULTICLOUD__ENVIRONMENT=production
MULTICLOUD__HTTP__HOST=127.0.0.1
MULTICLOUD__HTTP__PORT=8080
MULTICLOUD__DATABASE__URL='postgres://multicloud:STRONG_PASSWORD@127.0.0.1:5432/multicloud'
MULTICLOUD__DATABASE__MAX_CONNECTIONS=20
MULTICLOUD__REDIS__URL=redis://127.0.0.1:6379
MULTICLOUD__PROVIDER__CREDENTIAL_MASTER_KEY=BASE64_32_BYTE_KEY
MULTICLOUD__PROVIDER__CREDENTIAL_KEY_VERSION=1
RUST_LOG=info,multicloud=info
```

若数据库密码含 `@`、`:`、`/` 等 URL 保留字符，必须先进行 percent-encoding。

`RUST_LOG` 是连接数据库前的启动级别。迁移完成后，Platform Admin 可在 Web 顶栏选择 `error`、`warn`、`info`、`debug` 或 `trace`；选择会保存至 `platform_settings` 并立即 reload，无需重启。数据库只保存级别设置，application log 内容仍输出至 stdout/journald，不占用 PostgreSQL 空间。

## 首次初始化

生成 key、保护配置，并完成首次初始化：

```bash
openssl rand -base64 32
sudo chmod 600 /etc/multicloud.env
sudo sh -c 'set -a; . /etc/multicloud.env; exec /usr/local/bin/multicloud init'
```

`init` 会先自动应用 pending migrations，再在交互式终端创建首位 Platform Admin 与第一个 Organization。MultiCloud 使用规范化 email 作为唯一登录名，不另设容易混淆的 username；display name 只用于界面显示。密码使用隐藏输入与二次确认，不接受 command-line password 或环境变量，数据库只保存 Argon2 hash。

可在自动化安装中预填非敏感字段，密码仍会安全地从 TTY 读取：

```bash
sudo sh -c 'set -a; . /etc/multicloud.env; exec /usr/local/bin/multicloud init \
  --email admin@example.com \
  --display-name Administrator \
  --organization-slug primary \
  --organization-name "Primary Organization"'
```

后续普通用户由 Web 注册（默认关闭，由 Platform Admin 开启）。升级 binary 后先备份数据库，再执行 `multicloud migrate up`；migration 是增量且可重复调用的。

生产环境可创建专用用户，并写入 `/etc/systemd/system/multicloud.service`：

```ini
[Unit]
Description=MultiCloud Control Plane
After=network-online.target postgresql.service redis-server.service
Wants=network-online.target

[Service]
Type=simple
User=multicloud
Group=multicloud
EnvironmentFile=/etc/multicloud.env
ExecStart=/usr/local/bin/multicloud serve
Restart=on-failure
RestartSec=5s
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now multicloud
sudo systemctl status multicloud
journalctl -u multicloud -f
```

服务应以非 root 专用用户运行，并在前方配置 TLS reverse proxy。Redis 只监听本机或私有网络，PostgreSQL 与 Redis 都应纳入备份和监控。

Application log 的磁盘上限由日志后端管理。使用 journald 时，在 `/etc/systemd/journald.conf.d/limit.conf` 设置全局容量，例如：

```ini
[Journal]
SystemMaxUse=1G
SystemKeepFree=2G
MaxFileSec=7day
```

修改后执行 `sudo systemctl restart systemd-journald`。该限制影响整台主机的 journal，而非仅 MultiCloud。Audit Log 是另一类保存在 PostgreSQL 的不可变业务记录，由 Web 中的 tenant retention policy 与后续分区归档流程管理，不应按 application log 上限直接删除。

## 故障恢复

管理员失去访问权时，在服务器交互式终端加载相同配置：

```bash
sudo sh -c 'set -a; . /etc/multicloud.env; exec /usr/local/bin/multicloud recover-access admin@example.com'
```

该流程重设密码、撤销旧 session、恢复 Owner binding，并写入 Audit Event。
