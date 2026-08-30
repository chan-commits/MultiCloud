# Module 設計

## Workspace 邊界

- Apps：`api`、`worker`、`scheduler`、`agent`。
- Domain crates：`identity`、`organization`、`authorization`、`asset`、`provider`、`operation`、`ticket`、`chat`、`audit`、`billing`、`agent-control`、`notification`。
- Infrastructure crates：`persistence`、`messaging`、`websocket`、`observability`、`configuration`。
- `shared-kernel`：只放強型別 ID、Money、時間、分頁、correlation、idempotency 與 domain event 基礎型別。

## Bounded Context

| 模組 | 核心責任 | 主要資料所有權 |
|---|---|---|
| Identity | 使用者、登入、session、平台註冊政策、API token、SSO/MFA 擴充點 | users、platform_settings、identities、sessions、api_tokens |
| Organization | Organization lifecycle、membership、invitation、tenant switching | organizations、memberships、invitations |
| Authorization | permission catalog、role、binding、policy evaluation | permissions、roles、role_permissions、role_bindings |
| Asset/Resource | 業務 Asset、canonical Resource、Desired/Observed State、Drift 與 Reconciliation | assets、resources、resource states、drifts、reconciliation_tasks |
| Provider | account、credential lifecycle、adapter registry、capability、external mapping、sync cursor | provider_accounts、provider_credentials、external_resource_mappings、provider_sync_cursors |
| Operation | 長時間工作狀態、attempt/lease、冪等、進度、retry 與錯誤 | operations、provider_operation_attempts、outbox、inbox |
| Ticket | ticket lifecycle、comment、assignment、SLA、attachment metadata | tickets、comments、events、sla_policies |
| Chat | conversation、message、participant、read cursor | conversations、messages、participants、read cursors |
| Audit | append-only 管理與安全事件 | audit_logs |
| Billing | usage、pricing、charge、invoice、adjustment | billing tables |
| Agent Control | enrollment、identity、heartbeat、inventory、command | agent tables |
| Notification | in-app、email、WebSocket 與未來 webhook routing | notification preference/delivery records |

## 依賴規則

- Asset 只能依賴 Provider port 或發出 operation intent，不引用 OVH/Vultr/Cloudflare SDK。
- Resource 擁有 canonical state；Provider 擁有 external identity mapping，Asset 只建立業務關聯，不重複保存 external ID。
- Desired State 與 Observed State 分離，只有 managed fields 參與 drift comparison。
- Provider abstraction 不假設 Compute；Compute、DNS、Firewall、Certificate 由獨立 capability 表達。
- Provider 不直接修改其他 Domain 的 table；經 application service 或 event 交互。
- Billing 只消費 canonical usage，不讀 Provider 私有 response 計價。
- Chat 不以 Redis 作永久訊息儲存。
- Audit 消費其他 Domain event，但其他 Domain 不依賴 Audit 實作。
- Interface layer 只負責協定與輸入輸出，不承載業務規則。
- SeaORM Entity 只存在於 persistence boundary，不作為 Domain Entity 或 API DTO。

## RBAC 規則

Permission key 採 `domain.resource.action`，例如 `asset.vps.read`、`asset.vps.create`、`ticket.ticket.assign`。初期 scope 為 Organization，資料模型保留 Project/Asset scope。所有 use case 必須宣告 permission，不能只依賴前端隱藏操作。

Platform Admin 與 Organization RBAC 分離：只有 CLI 初始化的 Platform Admin 能修改全平台註冊政策；Organization Owner/Admin 不因此取得平台設定權限。

## Web UI 邊界

SvelteKit 負責 URL routing、持久 root layout 與頁面級 UI 邊界；Overview、Providers、Resources、Operations、Audit 均為獨立 route module。`App.svelte` 只組合 authentication、tenant-aware shell、跨頁協調與全域 overlay，透過 typed control-plane context 向 route 暴露共享狀態和操作。具備獨立狀態或表單生命週期的 UI 位於 `components/`。後續 Ticket、Chat、Agent 與 Billing 必須直接建立 route module，不再擴充手寫 view router。

UI 文案透過 `lib/i18n.svelte.ts` 管理 English、簡體中文與繁體中文；首次載入依 `navigator.languages` 選擇，未命中支援語系時回退 English，使用者選擇保存於 `localStorage`。新增頁面不得直接散落可見文案，應使用翻譯鍵並同步補齊兩種中文訊息。
