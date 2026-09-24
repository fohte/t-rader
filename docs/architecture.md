# backend のアーキテクチャ

backend の Rust crate は `backend/crates/` に配置する。`backend/migration/` は独立した migration crate として扱う。

## crate 構成

```text
backend/crates/
├── app/                  # 各 crate を組み立てて起動する bin
├── core/
│   ├── domain/           # 値、エンティティ、ドメインルール
│   └── application/      # ユースケースと port
├── entrypoints/          # アプリを外部から呼び出す入口
│   ├── frontend-api/
│   ├── agent-webhook/
│   ├── agent-mcp/
│   ├── control-plane-mcp/
│   └── scheduler/
└── gateways/             # アプリから外部システムへの接続
    ├── postgres/
    ├── jquants/
    ├── ibkr/
    ├── fred/
    ├── rss/
    ├── litellm/
    ├── kata-exec/
    └── t-rader-agent/
```

## crate の責務

| crate                | 責務                                                                                     |
| -------------------- | ---------------------------------------------------------------------------------------- |
| `core/domain`        | 値、エンティティ、ドメインルールを定義する。外部 crate に依存しない。                    |
| `core/application`   | ユースケースと port を定義する。port は application が必要とする機能を表す。             |
| `entrypoints/*`      | HTTP、MCP、webhook、定期実行などの入力を受け取り、application のユースケースを呼び出す。 |
| `gateways/*`         | application の port を実装し、外部システムとの入出力を担う。                             |
| `app`                | 各 crate を組み立てる composition root とする。                                          |
| `backend/migration/` | SeaORM migration を管理する。                                                            |

`app` は `rmcp` の session 管理、allowed hosts、access log など、複数の entrypoint に共通する MCP の配線も担う。

## crate 間の依存

crate 間の依存は `Cargo.toml` で次の関係に限定する。

| crate              | 依存先                                           |
| ------------------ | ------------------------------------------------ |
| `core/domain`      | なし                                             |
| `core/application` | `core/domain`                                    |
| `entrypoints/*`    | `core/application`, `core/domain`                |
| `gateways/*`       | `core/application`, `core/domain`                |
| `app`              | `backend/crates/` 内のすべての application crate |

`entrypoints/*` 同士、`gateways/*` 同士、および entrypoint と gateway の間は依存させない。`core/domain` と `core/application` から entrypoint や gateway に依存させない。`core/domain` は `reqwest` や `sea-orm` などの外部 crate にも依存しない。

## entrypoint と gateway の名前

entrypoint の crate 名は `<呼び出し元>-<手段>` とし、呼び出し元が時間である scheduler は 1 語にする。gateway の crate 名は接続先のシステム名にする。

HTTP path など外部との契約は crate 名と独立して管理する。たとえば `/mcp/strategy` と `/mcp/mgmt` は `agent-mcp` の crate 名とは別の契約である。

`postgres` gateway は PostgreSQL と TimescaleDB の双方を扱う。TimescaleDB 固有 SQL を含むため、両者を一つの gateway として扱う。

## port と外部形式の変換

**port** は application のユースケースが必要とする機能を表す interface とする。外部 API の endpoint ごとには分割しない。たとえば `DailyBarSource` は J-Quants と IBKR が実装し、`MarginSource` は J-Quants が実装する。実装が 1 つだけの場合も、依存逆転の境界として port を定義する。

**gateway** は外部システムの形式を domain の型へ変換して返す。gateway 固有のエラーも port が定めるエラーへ変換する。application が参照するテーブルには中立な名前とカラムを使い、外部システムの raw JSON を core から直接読まない。取り込み状態など gateway 内部だけで使うテーブルは、システム固有の名前を使ってよい。

## 権限とトランザクション

entrypoint はセッションなどから戦略スコープや管理者を表す権限型を組み立て、ユースケースへ渡す。権限は application のユースケースが検証する。

トランザクション境界は application のユースケースが `UnitOfWork` port を通して管理する。entrypoint は transaction を開始しない。

## エラーの境界

gateway は外部システム固有のエラーを port のエラーへ変換する。HTTP と MCP のエラー型、および application の結果から各プロトコルの応答への変換は entrypoint に置く。

## 参照するパターン

port と adapter の境界には [Hexagonal Architecture (Ports & Adapters)](https://alistair.cockburn.us/hexagonal-architecture/) を用いる。外部システムの形式を domain から隔離する境界は [Anti-Corruption Layer](https://learn.microsoft.com/en-us/azure/architecture/patterns/anti-corruption-layer) に対応する。`entrypoints` の構成は [Cosmic Python の project structure](https://www.cosmicpython.com/book/appendix_project_structure.html)、`gateways` の用語は [Fowler の Gateway pattern](https://martinfowler.com/eaaCatalog/gateway.html) を参照する。
