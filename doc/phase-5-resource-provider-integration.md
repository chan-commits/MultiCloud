# Phase 5：Resource Management 與 Real Provider Integration

Phase 5 使用 Phase 4 已建立的 Credential、Capability、Adapter、Registry 與 normalized error contracts，接入 Cloudflare、Vultr 與 OVH VPS。此階段不重做 Provider Foundation。

## 實作進度

- 已完成 canonical Resource、External Mapping、Desired/Observed State、metadata、Drift 與 Reconciliation schema/domain。
- 已完成 Provider Operation request、attempt、lease、retry/idempotency Worker execution。
- 已完成 Cloudflare API Token/Global API Key typed credential 與 risk metadata。
- 已完成 Cloudflare Zone/Record 分頁 inventory 與 DNS Record CRUD adapter contract。
- 已完成 Vultr API Token validation、Compute capability discovery、分頁 instance inventory，以及 get/create/start/stop/reboot/delete adapter contract。
- 已將 Vultr `halt` 正規化為平台 `stop`；操作成功後回寫 canonical Resource、mapping 與 Observed State。
- 已完成 OVH application credential（三段式簽名）、VPS list/get inventory 與 start/stop/reboot 非同步 Task 操作。
- 已完成 Resource/Desired State/Drift/Reconciliation REST API。
- 已完成 Fake inventory 重送 E2E：兩次 Operation/attempt 只建立一個 Resource mapping，Observed State 版本正常遞增。
- 待完成完整 UI 與真實 Provider opt-in 驗證。

## Canonical Resource Model

- `Resource`：平台對 Provider 資源的統一表示，例如 VPS、Zone、DNS Record。
- `ExternalResourceMapping`：Provider Account、external type/id 與 Resource 的唯一映射，是外部身份的單一真實來源。
- `Asset`：業務管理視角，可組合或關聯多個 Resource，不保存重複的 Provider external ID。
- `DesiredState`：使用者或平台期望且受管理的欄位。
- `ObservedState`：最近一次由 Provider 正規化取得的狀態、hash 與觀測時間。
- `ResourceMetadata`：Provider-specific、非核心且不直接參與 drift 比較的資料。

```text
External Provider Resource
          |
External Resource Mapping
          |
Canonical Resource ------- Asset
          |
Desired State / Observed State
```

## Provider Operation Executor

所有 Provider 寫操作採非同步流程：

```text
API transaction
  -> Operation + Outbox
  -> Worker claim / lease
  -> decrypt active credential
  -> resolve capability adapter
  -> Provider API
  -> persist mapping and observed state
  -> Operation terminal state + Outbox
```

每次執行寫入 immutable attempt，包含 attempt number、lease、provider request ID、masked request/result、normalized error、retry time 與完成時間。Operation 保存整體狀態，不覆蓋 attempt 歷史。

Idempotency 分層處理：

- tenant-scoped client idempotency key 阻止重複建立 Operation。
- Worker lease 阻止同一 Operation 同時執行。
- Provider 支援時傳遞 provider idempotency/request key。
- timeout 後重試 create 前先以 client reference、tag、request ID 或 inventory 查證結果。
- reconciliation task 以 resource、desired-state version 與 drift fingerprint 去重。

## Cloudflare Credential Extension

### API Token

- 預設且 Recommended。
- 支援 validation、permission scope identification 與 capability discovery。
- UI 說明 permissions 可被限制。

### Global API Key

- Legacy Compatibility、`risk_level=high`，不作預設選項。
- Credential envelope 是版本化 typed payload，包含 email 與 API key；兩者一起加密。
- UI 放在高級選項並顯示明確警告及二次確認。
- 建立、驗證、輪換、撤銷均要求 credential manage permission 並產生遮罩後的 security event。

API response 只返回 credential type、risk level、version、masked identifier 及 lifecycle timestamps，不返回 secret。明文不得進入 log、Operation snapshot、Provider error 或 event。

## Cloudflare DNS

Zone：分頁 list、details 與 external mapping。

DNS Record：分頁 list、create、update、delete；支援 A、AAAA、CNAME、TXT、MX。正規化 record name、TXT content、TTL、MX priority 與 proxied applicability，並保存受控 Provider metadata。

所有請求經 DNS capability adapter 與 Operation executor，不由 API handler 直接呼叫 Cloudflare。

## Compute Providers

Vultr 已實作 list/get/create/start/stop/reboot/delete，正規化 region、plan、OS、CPU、memory、disk、IP 與 provisioning/power state；REST 層只接受 canonical `stop`，adapter 才轉換成 Vultr `halt`。

OVH 此階段明確限定為 **OVH VPS API**：已實作 list/get、inventory、state sync，以及 API 實際提供的 start/stop/reboot。OVH power API 回傳非同步 Task，Operation 保存 Task 並由後續 inventory/reconciliation 收斂最終狀態。VPS API 未提供的 create/delete 不對外宣稱支援；不把 OVH Public Cloud Instance 或 Dedicated Server 混入同一 adapter/resource type。

## Synchronization

Scheduler 建立 sync Operation；Worker 依 cursor 分頁讀取 inventory，正規化後依 `(provider_account_id, external_type, external_id)` upsert mapping 與 Resource。相同 inventory 重送不得建立重複 Resource。

Provider 已刪除、暫時不可見與權限不足必須分開表示，不能在一次缺失後直接刪除 canonical Resource。

## Drift 與 Reconciliation

只比較 managed fields 的 Desired/Observed State，Provider 自動產生的 metadata 不參與 drift。狀態為 `in_sync`、`drifted`、`unknown` 或 `ignored`。

Reconciliation policy：

- `observe_only`
- `manual_approval`
- `automatic`

初期 DNS Record 採 manual approval、VPS power state 採 observe only；刪除型 drift 不自動修復。Task 必須包含 cooldown、desired-state version、stale observation check、max attempts 與唯一 drift fingerprint，避免修復循環。

## Event 與 Phase 6 邊界

Phase 5 只產生 audit-ready、已遮罩的 Domain Event；Phase 6 再投影至正式 `audit_logs`、query/export 與 retention。事件至少涵蓋 credential lifecycle、connection failure、resource discovery/change、operation requested/succeeded/failed、drift detected 與 reconciliation requested。

## 最小 UI

- 已建立 responsive、科技感 Command Center，並保留 Svelte 5 runes 與零新增前端依賴。
- 已接入 Bearer login、Organization workspace 切換；access token 只保存在 browser session，不做長期持久化。
- Dashboard 顯示 Provider、canonical Resource、Operation 與 desired/observed variance；Billing 與 time-series telemetry 明確標記為後續 Phase placeholder，不偽造資料。
- Provider Fabric 支援建立 Cloudflare、Vultr、OVH account、connection test 與 inventory sync。
- Cloudflare API Token 顯示 Recommended；Global API Key 放在 Legacy 選項、顯示 High Risk 警告。
- Resource Matrix 顯示 provider mapping、normalized lifecycle、Observed State、Drift 與 reconciliation approval。
- Compute Resource 可由 drawer 建立 start、stop、reboot Operation；所有操作仍通過既有 Operation Framework。
- Operation Stream 顯示 queue/progress/error，並允許取消尚未執行的 Operation。

UI 不直接保存 Provider secret，提交成功後立即清除 credential form。Resource REST DTO 額外返回 `provider_account_id`、`provider_kind` 與 `external_id`，讓 UI 使用明確 mapping 發出操作，不依名稱或 metadata 猜測 Provider。

後續 UI 增量：DNS Record editor、Provider credential rotation/disable、Desired State editor，以及正式 Observability/Billing 圖表。
