# 系統架構

## 架構形態

初期採 Modular Monolith，加上獨立 background worker、scheduler 與 Rust Agent。模組在同一 Rust workspace 內部署，但依 DDD 邊界隔離，日後可按負載或團隊邊界拆分服務。

## 部署單元

- `api`：Axum REST API、認證、Tenant Context、RBAC 與 WebSocket Gateway。
- `worker`：Provider 同步與操作、帳務計算、通知、Outbox 消費與失敗重試。
- `scheduler`：週期同步、帳單週期、SLA 與 Agent 健康檢查。
- `agent`：部署於 VPS，負責 heartbeat、inventory 與受限命令執行。
- `web`：SvelteKit static SPA，以檔案路由與持久 layout 組織 UI，透過 REST 與 WebSocket 使用 Control Plane；建置產物嵌入單一 Rust binary。

## 基礎設施責任

- PostgreSQL：Organization、RBAC、Asset、Ticket、Chat、Audit、Billing、Agent 與可靠事件的 source of truth。
- Redis：cache、rate limit、WebSocket presence、跨節點 Pub/Sub、短期 connection state 與工作協調。
- Object Storage：Ticket attachment、大型 Agent output 與匯出檔案；資料庫只保存 metadata。
- External Providers：OVH、Vultr、Cloudflare，由 Provider Adapter anti-corruption layer 接入。

## DDD 分層

每個 bounded context 分成：

- Domain：Aggregate、Entity、Value Object、Domain Service、Event、Repository port。
- Application：Command/Query use case、transaction boundary、authorization、DTO mapping。
- Infrastructure：SeaORM repository、Redis、Provider client、encryption、message transport。
- Interface：Axum handler、WebSocket handler、worker consumer。

依賴只能由外向內。Domain 層不得引用 web framework、ORM 或第三方 Provider model。

## 多租戶隔離

- 認證後建立不可變的 `TenantContext`，包含 user、active organization 與有效權限。
- 使用者可屬於多個 Organization，但每個 request/connection 只能有一個 active organization。
- Repository 必須將 `organization_id` 作為必要查詢條件。
- PostgreSQL RLS 作為第二道隔離防線。
- Redis key、WebSocket channel、job payload 與 audit event 都包含 organization namespace。
- Platform Admin 是明確的跨租戶角色，不由一般 Organization role 隱式取得。

## 一致性與事件

跨模組或外部系統操作使用 transactional outbox：同一資料庫 transaction 寫入 Aggregate 與 Outbox Event，worker 再執行外部操作。Consumer 使用 Inbox 或 idempotency key 去重。

外部操作由通用 Operation 記錄 `queued`、`running`、`succeeded`、`failed`、`cancelled`、`timed_out` 狀態。Provider API 呼叫不得放在長時間資料庫 transaction 中。

## Provider Adapter

Provider Domain 不直接綁定 Compute 或 VPS。Adapter 依能力切分：credential validation、inventory、compute lifecycle、DNS、firewall、certificate、usage ingestion。Provider Registry 依 `ProviderKind` 與 capability 解析 adapter，新增 Provider 不需要修改核心 Domain。

第三方 response 必須轉換成平台 canonical model；Provider 特有欄位可保存在受控 metadata，不得滲入 Asset Domain 核心模型。

Provider Account credential 採獨立 lifecycle 管理，包括加密保存、key version、輪替、撤銷、connection test 與 capability discovery。Phase 4 先以 fake adapter 建立 contract test，再用 Cloudflare API Token 驗證真實 credential 與 capability 流程；Compute inventory 與 resource lifecycle 延至 Phase 5。

## WebSocket

- 握手使用與 REST 相同的身份系統。
- 每次 subscription 都驗證 Organization 與 RBAC。
- 持久資料先寫 PostgreSQL，再經 Outbox/Redis 廣播。
- Redis Pub/Sub 負責多 API instance fan-out，不承擔訊息持久化。
- event envelope 包含 event ID、type、version、organization、channel/resource、trace ID 與時間/sequence。
- client 斷線後以 cursor 經 REST 或同步協定補取訊息。

## Rust Agent

Agent 主動連向 Control Plane，不開放入站管理 port。首次安裝使用短效 enrollment token，成功後換取獨立且可輪替的 identity。Command 具有 ID、期限、nonce、允許類型與冪等鍵；預設不提供任意 shell。所有命令、進度與結果進入 Audit Log。
