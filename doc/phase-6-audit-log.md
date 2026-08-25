# Phase 6：Audit Log

Phase 6 將既有 transactional outbox Domain Event 投影成 tenant-scoped、append-only Audit Log。Audit 消費其他 bounded context 的事件，核心 Domain 不依賴 Audit 實作。

## 實作範圍

- `audit_logs` 依 `occurred_at` range partition，建立當月 partition 與安全 default partition。
- database trigger 拒絕一般 `UPDATE`/`DELETE`，Audit record 只可追加。
- RLS 以 `organization_id` 隔離；`audit.log.read` 與 `audit.log.export` 分離授權。
- Worker 在 outbox transaction 內執行 idempotent projection，再進行 Redis fan-out；Redis 重試不會重複 Audit record。
- Worker 另有增量 backfill projector，會補齊 Phase 6 上線前已發布、但尚未投影的歷史 outbox event。
- 遞迴遮罩 password、token、secret、API/consumer/private key、authorization、credential、ciphertext 與 nonce。
- 遮罩器限制巢狀深度、array 數量與長字串大小，避免惡意 payload 放大。
- outcome 正規化為 `attempted/succeeded/failed/denied/cancelled`，severity 正規化為 `info/warning/critical`。
- Provider credential、Operation、Resource、Invitation、RBAC role/binding 與 authorization denied event 帶入 actor identity。

## Query 與 Export

- `GET /api/v1/audit-logs/`：依 action、target type、outcome、actor 篩選，以 `occurred_before + occurred_before_id` 穩定複合 cursor 分頁，單頁最多 200 筆。
- `GET /api/v1/audit-logs/export`：輸出最多 10,000 筆 sanitized CSV，並防止 spreadsheet formula injection。
- `GET /api/v1/audit-logs/retention`：讀取 Organization policy；未自訂時回傳 365/7 天預設值。
- `PUT /api/v1/audit-logs/retention`：Owner/Admin 以獨立 `audit.retention.manage` 權限更新 policy，並保存 before/after Audit Event。
- 查詢與匯出本身都建立 Audit Event，避免 Audit access 成為不可見的管理行為。
- Export 不包含完整 metadata，只包含時間、action、outcome、severity、actor、target 與 trace ID。

## Retention 基礎

`audit_retention_policies` 保存每個 Organization 的 log/export retention，並透過 tenant-scoped API 管理。Audit table 已具備月分區與 tenant/time indexes；後續 maintenance job 必須先建立未來 partition，再依 policy detach/archive/drop 過期 partition。初期預設保留 365 天，禁止低於 90 天。

## UI

Command Center 新增 Audit Stream：顯示 immutable event、actor、target、outcome、severity、trace 與摘要卡，支援 action/outcome 篩選、舊事件分頁及 RBAC-protected CSV export。UI 只接收已遮罩資料，不接觸 outbox 原始 payload。

Identity onboarding 採兩層邊界：首位平台管理員只能透過本機單一 binary 的 `multicloud init` 建立；平台初始化後，普通使用者可由 Web 註冊並建立自己的 Organization。Organization 建立與 Owner bootstrap 會產生 tenant audit event。

公開註冊政策預設關閉，只有 Platform Admin 可在已選定的 Organization context 中切換；事件保存 sanitized before/after，讓平台級安全變更在管理員租戶的 Audit Stream 可追蹤。

## 驗證

- recursive redaction unit test。
- CSV quote/formula injection unit test。
- 本地 migration 實際建立 partition、RLS 與 append-only trigger。
- Worker E2E 使用含假 token/consumer key 的 Domain Event，投影結果為 `[REDACTED]`，非敏感欄位保留。
- 對 Audit row 執行 `UPDATE` 由 database trigger 拒絕。
