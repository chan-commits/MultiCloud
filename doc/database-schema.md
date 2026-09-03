# Database Schema

## 全域規則

- 主要 ID 使用 UUID/UUIDv7；時間統一 UTC。
- 租戶資料必須包含 `organization_id`，常用索引以它作首欄。
- 租戶內 business key 採複合唯一索引。
- 核心可查詢欄位正規化；JSONB 僅放可演進、非核心或 Provider-specific metadata。
- Aggregate 需要競爭控制時加入 `version` 作 optimistic locking。
- SeaORM Entity 不直接暴露至 Domain 或 API。
- Audit、ledger、event 採不可變策略；可恢復業務資料才使用 soft delete。

## Identity、Organization、RBAC

- `users`：email、display name、status、password hash、verification timestamps，以及只由本機初始化授予的 `is_platform_admin`。
- `platform_settings`：singleton 平台設定；保存預設關閉的公開註冊開關、可動態 reload 的 application log level 及最後更新者。
- `user_identities`：user、identity provider、provider subject、metadata。
- `sessions`：refresh token hash、expiry、revocation、client metadata。
- `api_tokens`：user、organization、token hash、scopes、expiry、last used。
- `organizations`：slug、name、status、settings、timestamps。
- `organization_memberships`：organization、user、status、joined time；organization/user 唯一。
- `organization_invitations`：organization、email、token hash、inviter、expiry、acceptance。
- `permissions`：穩定 permission key 目錄。
- `roles`：organization、key、name、system flag。
- `role_permissions`：role/permission 複合主鍵。
- `role_bindings`：organization、role、subject type/id、scope type/id、creator。

## Provider 與 Asset

- `provider_accounts`：organization、kind、name、status、configuration、capabilities、validation state；不保存 credential。
- `provider_credentials`：organization、provider account、credential type、risk level、AES-GCM typed payload ciphertext/nonce、key/version、masked identifier、status、activation/revocation；每帳號只允許一笔 active credential。
- `resources`：organization、resource type、name、lifecycle、region、canonical attributes、timestamps；不直接保存 Provider external ID。
- `external_resource_mappings`：organization、provider account、resource、external type/id、client reference；`(provider_account_id, external_type, external_id)` 唯一。
- `resource_desired_states`：resource、version、managed fields、normalized state/hash、creator、timestamps。
- `resource_observed_states`：resource、source mapping、normalized state/hash、observed time；保存最新值，歷史 snapshot 依 retention 分表。
- `resource_metadata`：resource、source、namespace、Provider-specific metadata、observed time，不直接參與 drift。
- `resource_drifts`：resource、desired/observed version、fingerprint、status、diff、detected/resolved time。
- `reconciliation_tasks`：drift、policy、desired version、status、operation、cooldown、attempt、approval、timestamps；active drift fingerprint 唯一。
- `provider_operation_attempts`：operation、attempt number、lease owner/expiry、provider request ID、masked request/result、normalized error、retry/completion time；append-only attempt history。
- `provider_sync_cursors`：account、resource type、cursor、last sync、status/error。
- `assets`：organization、type、name、lifecycle、tags、metadata 與業務屬性。
- `asset_resources`：asset/resource 關聯與用途；一個 Asset 可組合多個 Resource。
- `vps_assets`：asset ID、hostname、plan/image、CPU、memory、disk、architecture、power state。
- `ip_addresses`：asset、address、family、visibility、primary flag、reverse DNS。
- `dns_zones`：asset ID、zone、status、nameservers、serial。
- `dns_records`：resource ID、zone resource ID、type、name、content、TTL、priority、proxied canonical details。
- `asset_state_snapshots`：asset、source、state、capture time，需 retention policy。

## Operation 與可靠訊息

- `operations`：organization、type、target、requester、idempotency、status、progress、error、timestamps。
- `outbox_events`：organization、aggregate、event type/version、payload、trace、occurred/published time、retry、dead-letter time；Worker 以 `SKIP LOCKED` 競爭派送。
- `inbox_messages`：organization、consumer/message ID 複合主鍵、processed time、result；claim 與資料庫副作用必須在同一 transaction。

Outbox 採 at-least-once delivery，Redis Pub/Sub 僅負責即時 fan-out；consumer 以 inbox 或下游 idempotency key 消除重複副作用。

## Ticket 與 Chat

- `tickets`：organization、tenant-scoped number、subject、description、status、priority、requester、assignee、SLA deadlines、version。
- `ticket_counters`：每个 Organization 的原子 ticket number allocator。
- `ticket_comments`：ticket、author、body、visibility、timestamps。
- `ticket_events`：ticket、event type、actor、data、time。
- `attachments`：owner、object storage key、filename、content type、size、checksum、uploader。
- `sla_policies`：organization、name、rules。
- `notifications`：organization、recipient、notification type、sanitized payload、read/created time。
- `conversations`：organization、type、subject、optional ticket、status、creator。
- `conversation_participants`：conversation、participant type/id、role、join/leave time。
- `chat_messages`：conversation、sender、client message ID、type、body、reply、metadata；client ID 用於去重。
- `conversation_read_cursors`：conversation、participant、last read message。

## Audit

- `audit_logs`：organization、actor、action、target、outcome、request/trace ID、client metadata、sanitized before/after、occurred time。
- 表為 append-only 且由 trigger 拒絕 update/delete；按 `occurred_at` range partition。
- `audit_retention_policies`：organization、log/export retention days、更新者與時間；預設 log retention 365 天，以 `audit.retention.manage` 獨立授權。

## Billing

- `billing_accounts`：organization、currency、status、billing profile。
- `price_books`：optional organization、currency、effective range、status。
- `prices`：price book、meter、pricing model、unit、decimal price、tiers、effective range。
- `usage_records`：organization、asset、provider、meter、quantity、period、source、deduplication key。
- `charges`：usage、description、quantity、unit price、amount、currency、period。
- `invoices`：billing account、number、status、currency、subtotal、tax、total、period、dates。
- `invoice_lines`：invoice、optional charge、description、quantity、unit price、amount、metadata。
- `billing_adjustments`：account、optional invoice、type、amount、reason、creator。

所有金額與數量採 decimal/numeric，不使用浮點數。

## Agent

- `agent_enrollment_tokens`：organization、asset、token hash、expiry、usage limits、revocation。
- `agents`：organization、asset、version、status、public key、credential version、last seen、platform、architecture。
- `agent_heartbeats`：agent、health、metrics summary、received time。
- `agent_inventory`：agent、version、inventory document、collection/receive time。
- `agent_commands`：agent、operation、type、payload、idempotency、status、issuer、expiry、ack/completion time。
- `agent_command_results`：command、sequence、status、sanitized output/error、time。

高頻 metrics 後續使用 time-series backend；PostgreSQL 僅保留摘要或降採樣資料。
