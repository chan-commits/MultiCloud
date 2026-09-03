# MultiCloud 設計文件

本目錄是 Multi Tenant Cloud Management Platform 的需求與架構決策基線。實作前若需求或設計變更，應同步更新對應文件。

## 文件索引

- [需求基線](requirements.md)：產品範圍、技術棧、設計原則與非功能需求。
- [系統架構](architecture.md)：部署單元、DDD 分層、多租戶與非同步架構。
- [模組設計](modules.md)：bounded context、責任邊界與依賴規則。
- [Database Schema](database-schema.md)：SeaORM/PostgreSQL 的資料模型基線。
- [核心流程](workflows.md)：登入、租戶切換、Provider、資產、Ticket、Chat、Billing 與 Agent 流程。
- [開發順序](development-roadmap.md)：分階段交付順序與完成條件。
- [Phase 1 API](phase-1-api.md)：Identity、Organization 與 TenantContext API 使用方式。
- [Phase 2 RBAC](phase-2-rbac.md)：Permission、Role、Binding 與授權矩陣。
- [Phase 3 Operation 與可靠事件](phase-3-operation-events.md)：Operation、Outbox、Inbox、Worker retry 與 Redis fan-out。
- [Phase 4 Provider Foundation](phase-4-provider-foundation.md)：Provider abstraction、加密 credential、Cloudflare 與 Fake adapter。
- [Phase 5 Resource 與 Real Provider](phase-5-resource-provider-integration.md)：canonical Resource、Cloudflare DNS、Vultr/OVH VPS、Operation executor 與 Drift。
- [Phase 6 Audit Log](phase-6-audit-log.md)：append-only audit projection、遮罩、query/export、partition 與 retention。
- [部署指南](deployment.md)：Podman/Docker Compose、本机 PostgreSQL/Redis、migration、首次初始化与恢复。

## 文件狀態

- 階段：Phase 6 已完成
- 程式碼：Phase 0–6 的 Identity、Organization、RBAC、Operation、Provider、Resource、UI 與 Audit 已實作
- 架構方式：Modular Monolith，保留日後拆分服務的邊界
- 最後更新：2026-08-19
