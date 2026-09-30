# backend のアーキテクチャ

backend の Rust crate は次の構成とする。`backend/migration/` は独立した migration crate として扱う。

## Clean Architecture の依存性ルール

backend の構成は [Clean Architecture の依存性ルール](https://blog.cleancoder.com/uncle-bob/2012/08/13/the-clean-architecture.html) を基準にする。ソースコードの依存は内側へ向け、`entrypoints` と `gateways` は `core/application` と `core/domain` に依存し、`core/application` は `core/domain` に依存する。許可する crate 間の直接依存は後述の表に定める。

## crate 構成

backend の crate は `backend/crates/<区分>/<crate>/` の 2 階層に配置する。`crates/` 直下には区分ディレクトリだけを置く。

```text
backend/crates/
├── apps/                 # 各 crate を組み立てて起動する bin
│   └── <app>/
├── core/
│   ├── domain/           # 値、エンティティ、ドメインルール
│   └── application/      # ユースケースと port
├── entrypoints/          # アプリを外部から呼び出す入口
│   ├── frontend-api/
│   ├── agent-webhook/
│   ├── agent-mcp/
│   ├── control-plane-mcp/
│   └── scheduler/
├── gateways/             # アプリから外部システムへの接続
│   ├── postgres/
│   ├── jquants/
│   ├── ibkr/
│   ├── fred/
│   ├── rss/
│   ├── litellm/
│   ├── kata-exec/
│   └── t-rader-agent/
└── libs/                 # 外部 crate と同じ扱いの自前ライブラリ
    ├── rate-limit/       # Redis を使った共有 rate limit
    └── test-macros/      # DB test 用 attribute macro
```

## crate の責務

| crate                | 責務                                                                                     |
| -------------------- | ---------------------------------------------------------------------------------------- |
| `core/domain`        | 値、エンティティ、ドメインルールを定義する。                                             |
| `core/application`   | ユースケースと port を定義する。port は application が必要とする機能を表す。             |
| `entrypoints/*`      | HTTP、MCP、webhook、定期実行などの入力を受け取り、application のユースケースを呼び出す。 |
| `gateways/*`         | application の port を実装し、外部システムとの入出力を担う。                             |
| `apps/*`             | 各 crate を組み立てる composition root とする。                                          |
| `libs/rate-limit`    | Redis を使って process 間で共有する rate limit と cooldown を提供する。                  |
| `libs/test-macros`   | DB test 用 proc macro を提供する。                                                       |
| `backend/migration/` | SeaORM migration を管理する。                                                            |

`apps/*` は `rmcp` の session 管理、allowed hosts、access log など、複数の entrypoint に共通する MCP の配線も担う。複数 crate を組み合わせる結合テストも `apps/*` に置く。

## crate 間の依存

`backend/crates/` 内の crate 間の直接依存は `Cargo.toml` で次の関係に限定する。

| crate              | 依存先                                                       |
| ------------------ | ------------------------------------------------------------ |
| `core/domain`      | なし                                                         |
| `core/application` | `core/domain`                                                |
| `entrypoints/*`    | `core/application`, `core/domain`                            |
| `gateways/*`       | `core/application`, `core/domain`, `libs/rate-limit`         |
| `apps/*`           | `backend/crates/` 内のすべての crate と `backend/migration/` |
| `libs/rate-limit`  | なし                                                         |
| `libs/test-macros` | なし                                                         |

`libs/rate-limit` は外部 crate への依存だけを持ち、`backend/crates/` 内の crate には依存しない。利用できるのは `gateways/*` と composition root (`backend`、将来の `apps/*`) のみとし、`core/*` と `entrypoints/*` からは依存させない。

`gateways/postgres` は `DatabaseHandle`、SeaORM entity 定義、repository 関数を提供する。trade など一部の集約では `core/application` が定義する `UnitOfWork`、repository、`ChangeHistoryPort` port も実装する。`test-support` feature だけが共有テスト DB の準備に必要な `migration` と `sqlx` を有効にする。`gateways/postgres` は `libs/test-macros` を dev-dependency として使い、backend と同じ DB test macro を利用できる。

## 集約のユースケース追加

`backend/src/services/use_cases.rs` の `UseCases` は、集約をまたいで共有する `DatabaseHandle`、`UnitOfWork`、変更履歴、戦略の存在確認を保持する。各集約のユースケースと Postgres adapter は `backend/src/services/use_cases/<aggregate>.rs` に置き、`impl UseCases` のメソッドで組み立てる。各メソッドは `Arc` に包んだ共通依存を clone して組み立てるため、必要なときに呼び出してよい。集約間の依存がある場合は、同じ `UseCases` のメソッドから組み立てる。

新しい集約を application に移すときは、port とユースケースを `backend/crates/core/application` に、Postgres adapter を `backend/crates/gateways/postgres` に追加し、対応する `services/use_cases/<aggregate>.rs` と `UseCases` のメソッドを実装する。`services/use_cases.rs` には新しい module をアルファベット順で宣言する。利用側はフィールドではなくメソッドを呼び出す。戦略タスクのように単独で必要な場合も、`build_use_cases` から同じメソッドを呼び出して組み立てる。

`core/application` の `lib.rs` には `pub mod` 宣言を置き、型を crate root に再エクスポートしない。利用側は `core_application::trade::...` のように module path から参照する。新しい port や集約を追加するときは、対応する module を `pub mod` で公開する。

HTTP の `AppState` と MCP の StrategyServer / MgmtServer は同じ `UseCases` を受け取る。HTTP handler は `state.use_cases` 経由で、StrategyServer は自身の `use_cases` 経由で対象のユースケースを呼び出す。MgmtServer にも同じ container を渡し、MCP server の組み立てを共通化する。transaction、変更履歴、strategy 存在確認には既存の共通 port と Postgres 実装を使う。HTTP では `PersistenceError` を `AppError` に変換する。

`entrypoints/*` 同士、`gateways/*` 同士、および entrypoint と gateway の間は依存させない。`core/domain` と `core/application` から entrypoint や gateway に依存させない。`core/domain` が直接依存してよい外部 crate は `chrono`, `rust_decimal`, `uuid`, `thiserror`, `jpholiday` (祝日の計算のみで I/O を持たない) と `serde` の derive に限る (テストでのみ使う dev-dependencies は対象外)。それ以外の外部 crate は依存させず、特に I/O や framework の crate (`sea-orm`, `reqwest`, `axum`, `rmcp`, `utoipa`, `tokio` など) は依存させない。

## entrypoint と gateway の名前

entrypoint の crate 名は `<呼び出し元>-<手段>` とし、呼び出し元が時間である scheduler は 1 語にする。gateway の crate 名は接続先のシステム名にする。

HTTP path など外部との契約は crate 名と独立して管理する。たとえば `/mcp/strategy` と `/mcp/mgmt` は `agent-mcp` の crate 名とは別の契約である。

`postgres` gateway は PostgreSQL と TimescaleDB の双方を扱う。TimescaleDB 固有 SQL を含むため、両者を一つの gateway として扱う。

`jquants` gateway は J-Quants API client を持ち、日足、銘柄マスタ、決算予定、信用残、空売り、財務情報、保有構造、バリュエーションの port を実装する。

## port と外部形式の変換

**port** は application のユースケースが必要とする機能を表す interface とする。外部 API の endpoint ごとには分割しない。たとえば `DailyBarSource` は J-Quants と IBKR が実装し、`MarginSource` は J-Quants が実装する。実装が 1 つだけの場合も、依存逆転の境界として port を定義する。

**gateway** は外部システムの形式を domain の型へ変換して返す。gateway 固有のエラーも port が定めるエラーへ変換する。application が参照するテーブルには中立な名前とカラムを使い、外部システムの raw JSON を core から直接読まない。取り込み状態など gateway 内部だけで使うテーブルは、システム固有の名前を使ってよい。

## 権限とトランザクション

entrypoint はセッションなどから戦略スコープや管理者を表す権限型を組み立て、ユースケースへ渡す。権限は application のユースケースが検証する。

トランザクション境界は application のユースケースが `UnitOfWork` port を通して管理する。entrypoint は transaction を開始しない。

ユースケースは `UnitOfWork` から transaction を取得し、同じ transaction を repository と変更履歴の port に渡す。すべての書き込みが成功した後に commit し、途中で失敗した場合は commit せず transaction を破棄する。gateway は opaque な transaction handle を自身の transaction 型へ変換し、SeaORM などの実装詳細を core に公開しない。

```rust
let transaction = unit_of_work.begin().await?;
let item = repository.update(&transaction, item).await?;
change_history
    .record(
        &transaction,
        ChangeHistoryRecord {
            actor: Actor::Human,
            target_kind: TargetKind::Trade,
            target_id: item.id,
            op: Op::Update,
            diff,
            summary: None,
        },
    )
    .await?;
unit_of_work.commit(transaction).await?;
```

## エラーの境界

gateway は外部システム固有のエラーを port のエラーへ変換する。HTTP と MCP のエラー型、および application の結果から各プロトコルの応答への変換は entrypoint に置く。
