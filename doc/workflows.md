# 核心流程

## 0. 首次初始化與管理權限恢復

1. 完成 database migration 後，在 Control Plane 主機的交互式終端執行 `just admin-init`（由單一 `multicloud` binary 的 `init` 子命令執行）。
2. CLI 原子建立首位 active User、Organization、active membership、system roles 與 Owner binding。
3. Password 僅由隱藏輸入讀取，不接受 command-line password，也不寫入 log；資料庫只保存 Argon2 hash。
4. 無法登入時執行 `just recover-access [email]`；使用者未指定時由 CLI 列出可恢復帳號。
5. 多租戶帳號必須明確選擇 Organization；沒有 membership 時必須提供 Organization UUID 並再次確認，不會跨租戶自動授權。
6. 恢復會重設 password、啟用 User/membership、確保 Owner binding，並撤銷該使用者所有既有 session。
7. 初始化與恢復均透過 transactional outbox 產生 security audit event；任何 event payload 均不包含 password/hash。
8. 這些能力只存在於本機 CLI，不提供 HTTP recovery endpoint。
9. 公開 Web 註冊預設關閉；首位 Platform Admin 初始化並登入後，可在 Web 明確開啟或再次關閉。普通使用者註冊後可在 Web 建立自己的 Organization，並成為該租戶 Owner。
10. 註冊政策是 platform scope，Organization Owner/Admin 無權修改；每次切換均產生 tenant-visible security audit event。

## 1. 登入與租戶切換

1. 使用者完成身份驗證，系統建立 session/token。
2. 系統讀取有效 Organization memberships。
3. 使用者選擇 active Organization。
4. API 驗證 membership，建立 TenantContext 與有效 RBAC permissions。
5. 後續 request 與 WebSocket connection 綁定該 Organization。
6. 切換 Organization 時重新建立 context；不得只信任 request body 的 organization ID。

## 2. 成員邀請與 RBAC

1. 管理者需具備 member management permission。
2. 系統建立有期限、只保存 hash 的 invitation token。
3. 受邀者接受後建立或啟用 membership。
4. 管理者建立 role binding；Authorization 模組計算有效權限。
5. 角色或 binding 變更產生 audit-ready Domain Event，並使相關 authorization cache 失效；Phase 6 再投影為 Audit Log。

## 3. Provider Account 連線

1. 管理者提交 Provider kind 與 credential。
2. 系統驗證 RBAC，使用 envelope encryption 儲存 credential。
3. Worker 透過對應 adapter 驗證 credential 與偵測 capabilities。
4. 成功後啟用 Provider Account；初次 inventory sync 由明確 request 或 Scheduler 建立 Operation。
5. 失敗則保存已遮罩錯誤，不回傳或記錄 secret。
6. 全流程產生已遮罩 security event；Phase 6 再投影為 Audit Log。

## 4. Provider Inventory Sync

1. Scheduler 建立 sync operation 與 Outbox Event。
2. Worker 取得 adapter，依 cursor 拉取外部資源。
3. Adapter 將第三方資料轉換為 canonical resource model。
4. 系統依唯一 External Resource Mapping upsert canonical Resource 與 Observed State，重送不得建立重複資料。
5. Asset 只建立業務關聯；Provider external ID 不重複保存到 Asset。
6. 比較 managed Desired/Observed State，標示 drift、unknown、missing 或 imported resource。
7. 更新 cursor 與同步時間，發布 Resource events。
8. 失敗採 rate-limit aware retry，超過上限進入 dead-letter/failed。

## 5. VPS 建立或生命週期操作

1. API 驗證 TenantContext、RBAC、輸入與 Provider capability。
2. Application transaction 建立 Operation 與 Outbox Event，立即回傳 operation ID。
3. Worker claim execution lease 並建立 immutable attempt，再使用 idempotency key 呼叫 Provider adapter。
4. Provider 接受後保存 request ID 與 External Resource Mapping；timeout 重試 create 前先查證 Provider state。
5. Worker polling 或接收 callback，更新進度與 canonical Resource Observed State。
6. 每次狀態變更經 Redis 向已授權 WebSocket subscribers 廣播。
7. 成功更新 canonical Resource；失敗保存 normalized、安全錯誤與 attempt/retry 狀態。
8. 所有管理操作與結果產生 audit-ready event，Phase 6 再建立 Audit Log。

## 6. Drift Detection 與 Reconciliation

1. Sync 保存新的 normalized Observed State 與 hash。
2. 系統只比較 Desired State 宣告的 managed fields，產生唯一 drift fingerprint。
3. `observe_only` 只發布事件；`manual_approval` 建立待核准 task；`automatic` 經安全 policy 後建立 Operation。
4. 執行前驗證 desired version 與 observed time 未過期，並套用 cooldown/max attempts。
5. 刪除型 drift 初期不自動修復；完成同步後重新觀測並關閉或更新 drift。

## 7. Ticket

1. 使用者建立 ticket，系統配置租戶內唯一 ticket number。
2. Ticket 進入初始狀態並產生 event。
3. 指派、優先級、comment、internal note 與狀態轉換均經 RBAC 與 Aggregate 規則。
4. SLA policy 計算首次回應及解決期限。
5. Domain Event 驅動 notification、WebSocket update 與 Audit Log。
6. Attachment 本體進 object storage，資料庫保存 metadata 與 checksum。

## 8. Live Chat

1. Client 以已認證 WebSocket 建立或訂閱 conversation。
2. Gateway 驗證 Organization、participant 身份與 read/write permission。
3. Client 送出具有 `client_message_id` 的訊息。
4. Server transaction 將訊息持久化並建立 Outbox Event；重送時以 client ID 去重。
5. Commit 後透過 Redis Pub/Sub 向各 API instance fan-out。
6. 收件者更新 read cursor；presence 與 typing indicator 只存 Redis 並設 TTL。
7. 斷線重連後以最後 cursor 補取遺漏訊息。

## 9. Audit Log

1. Use case 建立包含 actor、tenant、action、target、outcome 與 trace ID 的事件。
2. Audit consumer 移除 secret/敏感欄位，追加 audit record。
3. Audit record 不允許一般更新或刪除。
4. 查詢與匯出本身也必須受 RBAC 控制並留下 audit trail。

## 10. Billing

1. Provider/Asset/Agent usage 被轉換成 canonical usage record。
2. Deduplication key 阻止重複計量。
3. Rating job 依有效 price book 將 usage 轉成 charge。
4. 帳期結束後，以不可變 charge snapshot 產生 invoice 與 invoice lines。
5. Adjustment/credit 以新 ledger entry 表示，不回寫歷史金額。
6. Payment provider 後續以 adapter 接入，只更新付款狀態及外部 reference。

## 11. Agent Enrollment 與連線

1. 已授權管理者為指定 Asset 建立短效 enrollment token。
2. Agent 主動連線並提交 token 與本機 identity material。
3. Control Plane 驗證 Organization、Asset、期限與使用次數。
4. 系統建立 Agent identity，回傳可輪替憑證；token 隨即失效或扣除使用次數。
5. Agent 維持 outbound secure connection，定期送 heartbeat 與 inventory。
6. Offline 判斷由 scheduler 根據 last-seen threshold 執行。

## 12. Agent Command

1. 使用者提出受支援的 command，API 驗證 Asset scope 與 permission。
2. 系統建立 Operation、Agent Command 與 Audit Event。
3. Command 帶 ID、冪等鍵、期限及受限 payload，經已認證 channel 發送。
4. Agent 驗證 command 類型與期限，journal 後回覆 acknowledgement。
5. Agent 上報 progress/result；重複 command ID 不重複執行。
6. Control Plane 更新 Operation 並發布 WebSocket event。
7. 大型 output 放 object storage；資料庫與 log 保存遮罩後摘要。
