# Phase 4：Provider Foundation 與 Credential Integration

Phase 4 建立 capability-oriented Provider 平台與第三方憑證生命週期。Provider 不直接綁定 Compute；核心 Domain 不依賴 Cloudflare、Vultr 或 OVH SDK。

## 模組邊界

`multicloud-provider` 定義：

- `ProviderAdapter`：credential validation、capability discovery、inventory、operation。
- `ProviderRegistry`：以穩定 `ProviderKind` 動態解析 adapter；新增 adapter 不需要修改核心業務模型。
- `ProviderRuntime`：統一執行 validation/discovery、inventory 與 operation。
- `Capability`：Compute、DNS、Firewall、Certificate。
- `ProviderError`：Authentication、Authorization、RateLimited、NotFound、Conflict、InvalidRequest、Unavailable、Configuration、Unknown。
- `ProviderOperationRequest`：所有 operation 都攜帶 idempotency key。

Phase 4 的 Cloudflare adapter 實作 API Token verify 與 DNS capability discovery。DNS inventory/CRUD 留在 Phase 5。Fake adapter 提供 credential、inventory 與 operation 的整合契約測試。

## Credential 安全與生命週期

- API Token 使用 AES-256-GCM authenticated encryption。
- 每次加密使用新的 96-bit random nonce。
- Master key 只從 `MULTICLOUD__PROVIDER__CREDENTIAL_MASTER_KEY` 注入，不寫入 Git 或 PostgreSQL。
- `provider_credentials` 保存 ciphertext、nonce、key version 與 credential version。
- 同一 Provider Account 只能有一笔 active credential。
- rotate 在同一 transaction 中 revoke 舊版本、建立新版本，並將帳號改回 `pending_validation`。
- disable 在同一 transaction 中停用帳號並 revoke 當前 active credential。
- 明文 credential 不實作 `Debug`，離開作用域時 zeroize；API response、event 與 log 永不包含 credential。

產生本地 master key：

```bash
openssl rand -base64 32
```

`key_version` 為未來 keyring/re-encryption 保留；目前啟動中的 API 必須使用能解密 active credential 的版本。

## Provider Account API

- `GET /api/v1/providers`
- `POST /api/v1/providers`
- `GET /api/v1/providers/{account_id}`
- `POST /api/v1/providers/{account_id}/credentials`
- `POST /api/v1/providers/{account_id}/connection-test`
- `POST /api/v1/providers/{account_id}/disable`

建立帳號只會加密保存 credential，狀態为 `pending_validation`。Connection test 建立 `provider.connection_test` Operation，调用 adapter 后原子更新 Operation、account status、capabilities 及 outbox event。

权限：

- `provider.account.read`
- `provider.account.manage`
- `provider.connection.test`

## Retry 與 Rate Limit

Adapter 將 HTTP 429 正規化為 `RateLimited`，保存 `retryable=true` 與可用的 `Retry-After` 秒數。Provider operation worker 在 Phase 5 使用這些欄位配合 Operation/outbox retry policy 排程重試，Domain 不直接依賴 HTTP status。

## 驗證結果

- Fake adapter：credential validation、四類 capability、inventory 與 operation contract 通過。
- Cloudflare adapter：本地 HTTP mock 完成 API Token validation 與 DNS capability discovery，無需真實 Token。
- API E2E：Provider Account 建立、connection Operation、capability 保存與 credential rotation 通過。
- Database：舊 credential revoked、新 credential active，ciphertext 未保存明文。
- Migration：Phase 4 down/up 可逆。
