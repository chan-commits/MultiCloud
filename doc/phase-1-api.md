# Phase 1 API

## Authentication

### `POST /api/v1/auth/register`

建立使用者。输入包含 `email`、至少 12 字元的 `password` 与 `display_name`。Email 会正规化为小写，密码使用 Argon2id hash 保存。

### `POST /api/v1/auth/login`

验证帐号密码并签发 256-bit 随机 Bearer token。数据库只保存 SHA-256 token hash，session 默认 30 天到期。

### `POST /api/v1/auth/logout`

需要 `Authorization: Bearer <token>`，将当前 session 标记为 revoked。

## Organization

### `POST /api/v1/organizations`

需要 Bearer token。建立 Organization，并在同一 transaction 内把建立者加入为 active member。

### `GET /api/v1/organizations`

需要 Bearer token。只返回当前使用者具有 active membership 的 Organization。

## Invitation

### `POST /api/v1/invitations`

需要 Bearer token、`X-Organization-Id` 与 `organization.invitation.manage` 权限。建立七天有效的 invitation；token 仅在建立时返回，数据库只保存 hash。

### `POST /api/v1/invitations/accept`

需要 Bearer token。输入包含 `organization_id` 与 invitation `token`。只有 invitation email 与登入使用者 email 相同才能接受。

## TenantContext

### `GET /api/v1/tenant/context`

需要：

- `Authorization: Bearer <token>`
- `X-Organization-Id: <uuid>`

服务器先验证 session，再于 tenant-scoped transaction 设置 PostgreSQL `app.user_id` 与 `app.organization_id`，最后验证 active membership 并返回有效 permission keys。不存在或不属于使用者的 Organization 返回 `403 Forbidden`。

后续所有 tenant-scoped use case 都必须沿用相同 transaction context，不得直接信任客户端提供的 Organization ID。
