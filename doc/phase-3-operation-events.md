# Phase 3：Operation 與可靠事件

Phase 3 建立跨模組長時間任務與可靠事件的共用基礎，供 Provider、資源同步、Audit、Ticket、Chat 與 Agent 使用。

## Operation

`operations` 是租戶範圍內的非同步工作追蹤模型。建立 Operation 時必須提供 tenant-scoped `idempotency_key`，並在同一個 PostgreSQL transaction 中寫入初始 outbox event。

狀態模型：

```text
queued -> running -> succeeded
   |         |----> failed
   |         |----> cancelled
   `--------------> cancelled
```

Phase 3 提供以下唯讀與取消 API：

- `GET /api/v1/operations`
- `GET /api/v1/operations/{operation_id}`
- `POST /api/v1/operations/{operation_id}/cancel`

列表與明細需要 `operation.operation.read`；取消需要 `operation.operation.cancel`。取消目前只接受 `queued` Operation，並原子寫入 `operation.cancelled` event。

## Transactional Outbox

產生業務狀態與 event 的 use case 必須共用同一個 database transaction。Worker 使用 `FOR UPDATE SKIP LOCKED` 競爭 pending event，成功後標記 `published_at`；失敗時採 capped exponential backoff，最多嘗試十次，之後寫入 `dead_lettered_at`。

事件以 JSON envelope 發布至：

```text
multicloud:events:{organization_id}
```

Redis Pub/Sub 是低延遲 fan-out transport，不是持久 event store。PostgreSQL outbox 才是發布狀態的真實來源。事件交付語義是 at-least-once，consumer 必須實作冪等。

## Inbox Idempotency

Consumer 在處理 event 前使用 `(consumer, message_id)` claim inbox message。claim 與 consumer 的 PostgreSQL 副作用必須放在同一個 transaction：

1. 嘗試建立 inbox row。
2. 已存在時停止處理並回傳既有結果。
3. 首次 claim 時執行業務副作用。
4. 一起 commit inbox row 與業務變更。

這個模式可讓相同 event 重複投遞而不重複執行資料庫副作用。非 PostgreSQL 外部副作用仍須使用下游 idempotency key。

## Tenant 與安全邊界

- `operations` 強制啟用 PostgreSQL RLS，API 仍同時套用 repository scope。
- `outbox_events`、`inbox_messages` 具有 tenant RLS policy，但不使用 `FORCE ROW LEVEL SECURITY`，讓目前以 migration owner 執行的內部 Worker 可以跨租戶 dispatch。
- API 不提供直接讀寫 outbox/inbox 的 endpoint。
- Production hardening 階段應建立最小權限的獨立 Worker database role，取代 owner bypass。

## Phase 3 驗證

- Operation 查詢、queued cancellation 與取消 event 已完成端到端驗證。
- Invitation domain event 已驗證可由 Worker 發布到真實 Redis subscriber。
- Redis 中斷時 event 會保留並增加 retry attempt；Redis 恢復後可成功發布。
- Inbox primary key 保證同一 consumer 對相同 message 只 claim 一次。
