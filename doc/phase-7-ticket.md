# Phase 7：Ticket

Phase 7 建立 Organization-scoped 支援工單 bounded context。Ticket Domain 僅負責 lifecycle、priority 與 SLA deadline 規則；Axum、SeaORM、通知投影及 Audit event 位於 application/infrastructure 邊界。

## Domain 與資料模型

- `ticket_counters` 原子配置 tenant-local ticket number。
- `tickets` 保存 requester、assignee、status、priority、optimistic version 與 SLA timestamps。
- `ticket_comments` 支援 public/internal visibility；internal comment 僅 `ticket.ticket.manage` 可寫入及讀取。
- `attachments` 保存 object-storage registration metadata、100 MiB 上限與 SHA-256；二進位內容不進 PostgreSQL。
- `sla_policies` 支援每租戶唯一 default policy；未設定時使用 response 60 分鐘、resolution 1440 分鐘。
- `notifications` 保存 assignment/comment recipient inbox，並提供 tenant/user-scoped read API。

所有表均帶 `organization_id` 並啟用 FORCE RLS。Ticket 更新使用 `version` optimistic concurrency；跨租戶 ID 即使被猜中也無法讀寫。

## REST API

- `GET/POST /api/v1/tickets/`
- `GET/PATCH /api/v1/tickets/{ticket_id}`
- `GET/POST /api/v1/tickets/{ticket_id}/comments`
- `POST /api/v1/tickets/{ticket_id}/attachments`
- `GET/POST /api/v1/tickets/sla-policies`
- `GET /api/v1/tickets/notifications`
- `POST /api/v1/tickets/notifications/{id}/read`

权限拆分为 read、create、comment、manage 与 SLA manage。Owner/Admin 拥有全部能力；Member 可建立、读取及公开评论；Viewer 只读。Assignment 目标必须是 active Organization member。

## Lifecycle 與事件

状态为 `open → in_progress/waiting_on_customer → resolved → closed`，允许显式 reopen，禁止从 closed 直接进入处理中。创建、更新、评论、附件与 SLA 变更均写入 transactional outbox，随后投影到 Phase 6 Audit Log。Assignment 与评论会产生持久 notification。

## UI 與驗證

SvelteKit `/tickets` 页面提供工单创建、tenant-local number、priority/status、详情、SLA deadline 和评论。Domain tests 覆盖非法状态转换及 SLA deadline；全新 PostgreSQL migration 验证所有表和权限，非 superuser 数据库角色下的跨 Organization RLS 探针返回零行。
