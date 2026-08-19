# Phase 2 RBAC

## 权限模型

权限 key 固定采用 `domain.resource.action`。Permission catalog 属于平台级稳定资料，Role 与 Role Binding 属于 Organization 租户资料。

Phase 2 权限目录：

| Permission | 用途 |
|---|---|
| `organization.organization.read` | 读取 Organization |
| `organization.organization.update` | 更新 Organization |
| `organization.member.read` | 读取成员 |
| `organization.member.manage` | 管理成员 |
| `organization.invitation.manage` | 建立与撤销邀请 |
| `authorization.role.read` | 读取 permission catalog 与 roles |
| `authorization.role.manage` | 建立 custom role |
| `authorization.binding.manage` | 建立与删除 role binding |

## 系统角色

| Role | 权限 |
|---|---|
| Owner | 全部 Phase 2 权限 |
| Admin | 全部 Phase 2 权限 |
| Member | Organization 与成员读取 |
| Viewer | Organization 读取 |

Organization 建立时会在同一 transaction 建立四个系统角色，并为建立者配置 Owner binding。Invitation 接受后，受邀者自动获得 Member binding。系统角色 key 保留，不能作为 custom role key。

## 授权流程

1. Bearer token 解析为有效 session。
2. `X-Organization-Id` 与 active membership 建立 TenantContext。
3. 在同一个数据库 transaction 设置 `app.user_id`、`app.organization_id`。
4. Policy evaluator 根据 user、organization、scope、role 与 permission 计算权限。
5. 权限不足返回 `403`，不会执行业务 mutation。
6. 通过后在同一个 transaction 执行业务操作，降低授权与写入之间的竞态窗口。

Policy evaluator 不依赖 REST payload，可由后续 WebSocket subscription/command handler 共用。

## API

所有 API 均需要 Bearer token 与 `X-Organization-Id`。

- `GET /api/v1/rbac/permissions`：读取 permission catalog，需要 `authorization.role.read`。
- `GET /api/v1/rbac/roles`：读取 Organization roles 与权限，需要 `authorization.role.read`。
- `POST /api/v1/rbac/roles`：建立 custom role，需要 `authorization.role.manage`。
- `POST /api/v1/rbac/bindings`：为 active member 配置 role，需要 `authorization.binding.manage`。
- `DELETE /api/v1/rbac/bindings/{binding_id}`：删除 binding，需要 `authorization.binding.manage`。

Owner 不可删除自己的 Owner binding，避免 Organization 在没有 owner 的情况下被锁死。

## 租户隔离

`roles`、`role_permissions`、`role_bindings` 都包含 `organization_id` 并启用强制 PostgreSQL RLS。Binding 的 `scope_id` 必须与当前 Organization 相同。外部传入的 role、binding 或 member ID 必须在 tenant transaction 内重新验证。
