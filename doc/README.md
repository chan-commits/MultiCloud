# MultiCloud 設計文件

本目錄是 Multi Tenant Cloud Management Platform 的需求與架構決策基線。實作前若需求或設計變更，應同步更新對應文件。

## 文件索引

- [需求基線](requirements.md)：產品範圍、技術棧、設計原則與非功能需求。
- [系統架構](architecture.md)：部署單元、DDD 分層、多租戶與非同步架構。
- [模組設計](modules.md)：bounded context、責任邊界與依賴規則。
- [Database Schema](database-schema.md)：SeaORM/PostgreSQL 的資料模型基線。
- [核心流程](workflows.md)：登入、租戶切換、Provider、資產、Ticket、Chat、Billing 與 Agent 流程。
- [開發順序](development-roadmap.md)：分階段交付順序與完成條件。

## 文件狀態

- 階段：設計第一版
- 程式碼：尚未開始
- 架構方式：Modular Monolith，保留日後拆分服務的邊界
- 最後更新：2026-08-19

