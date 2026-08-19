# 開發順序

## 當前進度

- Phase 0：已完成（2026-08-19）
- Phase 1：已完成（2026-08-19）
- Phase 2：已完成（2026-08-19）
- Phase 3：已完成（2026-08-19）
- Phase 4：已完成（2026-08-19）
- Phase 5：待開始

## Phase 0：工程基礎

建立 Cargo workspace、Svelte app、`justfile`、configuration、logging/tracing、PostgreSQL/Redis、本地環境、migration 與 CI 基線。

完成條件：API、worker、scheduler 可獨立啟動並提供健康檢查。

## Phase 1：Identity 與 Organization

實作 user、session、Organization、membership、invitation、TenantContext、repository scoping 與 PostgreSQL RLS。

完成條件：自動化測試證明一般使用者無法跨租戶讀寫。

## Phase 2：RBAC

實作 permission catalog、system/custom role、binding、policy evaluator，以及 REST/WebSocket 共用授權。

完成條件：每個 use case 宣告 permission，具備允許/拒絕測試矩陣。

## Phase 3：Operation 與可靠事件

實作通用 Operation、transactional outbox、inbox idempotency、worker retry/dead-letter、Redis fan-out。

完成條件：可重複投遞事件而不造成重複副作用。

## Phase 4：Provider Foundation 與 Credential Integration

實作 Provider 基礎平台與第三方憑證管理能力。

包含：

- encrypted Provider Account
- credential lifecycle management
- credential validation / connection test
- capability model
- provider registry
- provider adapter interface
- sync framework
- operation adapter framework
- provider error normalization
- rate-limit handling
- fake provider adapter

Provider abstraction 原則：

- Provider 不直接綁定 Compute 資源。
- 以 capability model 描述 Provider 能力。
- 支援 Compute、DNS、Firewall、Certificate 等不同資源類型。
- 新增 Provider 不需要修改核心 Domain。

第一個真實 Provider 整合採 Cloudflare：

- API Token credential
- Token validation
- Account capability discovery

支援流程：

1. 建立 Provider Account。
2. 加密保存 credential。
3. 測試 Provider connection。
4. 取得並保存 Provider capabilities。

完成條件：

- 使用 fake provider 完成 credential、inventory 與 operation 的整合測試。
- 使用 Cloudflare adapter 完成 credential 驗證與 capability discovery 測試。
- Provider adapter 可獨立新增，不影響核心業務邏輯。

## Phase 5：Resource Management 與 Real Provider Integration

實作統一資源模型並接入真實 Provider。

Canonical resource model：

- Asset
- Resource
- External Resource Mapping
- Resource State
- Drift Detection

Provider 資源同步範圍：

- Compute Provider：Vultr、OVH
- Network Provider：Cloudflare DNS

實作內容：

- Resource inventory sync
- External ID mapping
- Resource lifecycle operation
- Provider state reconciliation
- Drift detection

VPS 資源操作：

- list
- create
- start
- stop
- reboot
- delete

Cloudflare DNS 操作：

- list zones
- list DNS records
- create record
- update record
- delete record

所有 Provider 操作必須通過：

- Operation framework
- retry mechanism
- idempotency handling
- audit event pipeline

完成條件：

- VPS Provider 可完成完整 inventory 同步。
- VPS lifecycle 操作可通過 Operation 全程追蹤。
- Cloudflare DNS 資源可同步並執行 CRUD 操作。
- Provider drift 可被檢測並產生 reconciliation task。

## Phase 6：Audit Log

完成 event-to-audit pipeline、敏感欄位遮罩、query/export、partition 與 retention 基礎。

完成條件：身份、RBAC、Provider、Asset 的安全及管理操作都有完整軌跡。

## Phase 7：Ticket

實作 ticket lifecycle、comment、attachment、assignment、SLA、notification 與權限控制。

完成條件：狀態轉換、SLA 與 tenant isolation 具備整合測試。

## Phase 8：Live Chat 與 WebSocket

實作 conversation/message persistence、subscription authorization、Redis fan-out、read cursor、reconnect recovery、presence 與 typing。

完成條件：多 API instance 下不遺失持久訊息，重連可以補訊息。

## Phase 9：Rust Agent

實作 enrollment、identity/credential rotation、heartbeat、inventory、受限 command、ack/progress/result、斷線恢復與更新策略。

完成條件：Agent 不需開入站 port，command 可冪等執行且全程可稽核。

## Phase 10：Billing

實作 canonical meter、usage ingestion、price book、rating、charge、invoice、adjustment 與 Provider cost reconciliation。

完成條件：相同 usage 重送不重複收費，已開立 invoice 不被後續價格變動改寫。

## Phase 11：Production Hardening

完成 contract tests、隔離測試、backup/restore、secret rotation、rate limiting、WebSocket backpressure、partition/retention、監控告警、供應鏈安全及故障演練。
