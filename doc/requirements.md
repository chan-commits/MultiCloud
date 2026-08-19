# 需求基線

## 產品目標

建立一個多租戶雲端管理平台，讓不同 Organization 能在嚴格隔離下管理雲端帳號、VPS、DNS、支援工單、即時聊天、帳務及安裝於 VPS 的 Rust Agent。

## 技術棧

### Backend

- Rust
- Axum
- Tokio
- SeaORM
- PostgreSQL
- Redis
- WebSocket

### Frontend

- Svelte
- TypeScript
- Vite
- TailwindCSS

### Tooling

- `just` 作為統一的開發、測試、migration、lint 與啟動入口

## 功能範圍

1. 多租戶 Organization 與 membership。
2. Organization scoped RBAC。
3. VPS Asset Management。
4. Provider Adapter System：OVH、Vultr、Cloudflare。
5. Ticket System。
6. Live Chat。
7. Append-only Audit Log。
8. Billing Architecture。
9. 安裝於受管 VPS 的 Rust Agent。

## 架構要求

- 採 Domain Driven Design 與 bounded context。
- Domain 不依賴 Axum、SeaORM、Redis 或特定 Provider SDK。
- 各模組有明確責任、資料所有權與依賴方向。
- Provider 使用 capability-oriented trait abstraction，避免單一巨大介面。
- PostgreSQL 是業務資料唯一真實來源。
- Redis 僅用於快取、限流、短期狀態、presence、Pub/Sub 與工作協調。
- 外部 Provider 操作採非同步 operation、transactional outbox、重試與冪等設計。
- Chat message 必須先持久化，再透過 WebSocket 廣播。
- 所有租戶資料預設帶 `organization_id`，並以 repository scope 加 PostgreSQL RLS 雙重隔離。

## 安全與非功能需求

- Provider credentials 採 envelope encryption，不得明文儲存或寫入 log。
- REST 與 WebSocket 共用 authentication 與 authorization policy。
- 所有敏感及管理操作須留下 audit trail。
- Agent 採 outbound-only、安全 enrollment、獨立 identity、憑證輪替及受限 command catalog。
- 所有外部操作須支援 idempotency、timeout、retry、dead-letter 與可觀測性。
- 金額使用 decimal，不使用浮點數。
- 高寫入量資料須預留 partition、retention 與 archive 策略。

## 第一階段文件範圍

目前只定義：系統架構、模組設計、Database Schema、核心流程與開發順序；不包含業務程式碼。

