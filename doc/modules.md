# Module 設計

## Workspace 邊界

- Apps：`api`、`worker`、`scheduler`、`agent`。
- Domain crates：`identity`、`organization`、`authorization`、`asset`、`provider`、`operation`、`ticket`、`chat`、`audit`、`billing`、`agent-control`、`notification`。
- Infrastructure crates：`persistence`、`messaging`、`websocket`、`observability`、`configuration`。
- `shared-kernel`：只放強型別 ID、Money、時間、分頁、correlation、idempotency 與 domain event 基礎型別。

## Bounded Context

| 模組 | 核心責任 | 主要資料所有權 |
|---|---|---|
| Identity | 使用者、登入、session、API token、SSO/MFA 擴充點 | users、identities、sessions、api_tokens |
| Organization | Organization lifecycle、membership、invitation、tenant switching | organizations、memberships、invitations |
| Authorization | permission catalog、role、binding、policy evaluation | permissions、roles、role_permissions、role_bindings |
| Asset | 平台 canonical VPS、IP、DNS、network/firewall inventory | assets 與各 asset detail tables |
| Provider | account、credential lifecycle、adapter registry、capability、sync contract、外部操作 | provider_accounts、resource_refs、provider_operations |
| Operation | 長時間工作狀態、冪等、進度、錯誤 | operations、outbox、inbox |
| Ticket | ticket lifecycle、comment、assignment、SLA、attachment metadata | tickets、comments、events、sla_policies |
| Chat | conversation、message、participant、read cursor | conversations、messages、participants、read cursors |
| Audit | append-only 管理與安全事件 | audit_logs |
| Billing | usage、pricing、charge、invoice、adjustment | billing tables |
| Agent Control | enrollment、identity、heartbeat、inventory、command | agent tables |
| Notification | in-app、email、WebSocket 與未來 webhook routing | notification preference/delivery records |

## 依賴規則

- Asset 只能依賴 Provider port 或發出 operation intent，不引用 OVH/Vultr/Cloudflare SDK。
- Provider abstraction 不假設 Compute；Compute、DNS、Firewall、Certificate 由獨立 capability 表達。
- Provider 不直接修改其他 Domain 的 table；經 application service 或 event 交互。
- Billing 只消費 canonical usage，不讀 Provider 私有 response 計價。
- Chat 不以 Redis 作永久訊息儲存。
- Audit 消費其他 Domain event，但其他 Domain 不依賴 Audit 實作。
- Interface layer 只負責協定與輸入輸出，不承載業務規則。
- SeaORM Entity 只存在於 persistence boundary，不作為 Domain Entity 或 API DTO。

## RBAC 規則

Permission key 採 `domain.resource.action`，例如 `asset.vps.read`、`asset.vps.create`、`ticket.ticket.assign`。初期 scope 為 Organization，資料模型保留 Project/Asset scope。所有 use case 必須宣告 permission，不能只依賴前端隱藏操作。
