# public.annotation

## Description

戦略に属する銘柄などの対象へ付与したテキスト注釈を保持する。

## Columns

| Name              | Type                     | Default                     | Nullable | Children | Parents                               | Comment                                           |
| ----------------- | ------------------------ | --------------------------- | -------- | -------- | ------------------------------------- | ------------------------------------------------- |
| id                | uuid                     |                             | false    |          |                                       |                                                   |
| strategy_id       | uuid                     |                             | true     |          | [public.strategy](public.strategy.md) | 注釈を所有する戦略。戦略に属さない注釈では null。 |
| target_symbol     | varchar                  |                             | false    |          |                                       | 注釈対象を識別する銘柄・参照の ID。               |
| target_kind       | varchar                  |                             | false    |          |                                       | 注釈対象の種類。                                  |
| timestamp         | timestamp with time zone |                             | false    |          |                                       | 注釈を紐づける時刻。                              |
| price             | numeric                  |                             | true     |          |                                       | 注釈を紐づける価格。                              |
| text              | text                     |                             | false    |          |                                       | 注釈本文。                                        |
| status            | varchar                  | 'unread'::character varying | false    |          |                                       | 注釈のレビュー状態。                              |
| linked_note_id    | uuid                     |                             | true     |          | [public.note](public.note.md)         | 注釈に関連付けたノート。                          |
| created_by_kind   | varchar                  |                             | false    |          |                                       | 注釈を作成した主体の種別。                        |
| created_at        | timestamp with time zone | CURRENT_TIMESTAMP           | false    |          |                                       |                                                   |
| updated_at        | timestamp with time zone | CURRENT_TIMESTAMP           | false    |          |                                       |                                                   |
| execution_step_id | uuid                     |                             | true     |          |                                       | 注釈を作成したタスク実行ステップの UUID。         |
| execution_task_id | text                     |                             | true     |          |                                       | 注釈を作成したタスク実行の識別子。                |

## Constraints

| Name                                   | Type        | Definition                                                                                                                                  |
| -------------------------------------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| annotation_created_by_kind_check       | CHECK       | CHECK (((created_by_kind)::text = ANY ((ARRAY['human'::character varying, 'llm'::character varying])::text[])))                             |
| annotation_status_check                | CHECK       | CHECK (((status)::text = ANY ((ARRAY['approved'::character varying, 'unread'::character varying, 'rejected'::character varying])::text[]))) |
| annotation_strategy_id_execution_check | CHECK       | CHECK (((strategy_id IS NOT NULL) OR ((execution_step_id IS NULL) AND (execution_task_id IS NULL))))                                        |
| annotation_strategy_id_fkey            | FOREIGN KEY | FOREIGN KEY (strategy_id) REFERENCES strategy(id) ON DELETE CASCADE                                                                         |
| annotation_linked_note_id_fkey         | FOREIGN KEY | FOREIGN KEY (linked_note_id) REFERENCES note(id) ON DELETE SET NULL                                                                         |
| annotation_pkey                        | PRIMARY KEY | PRIMARY KEY (id)                                                                                                                            |

## Indexes

| Name                                         | Definition                                                                                                                  |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| annotation_pkey                              | CREATE UNIQUE INDEX annotation_pkey ON public.annotation USING btree (id)                                                   |
| idx_annotation_target_symbol_timestamp       | CREATE INDEX idx_annotation_target_symbol_timestamp ON public.annotation USING btree (target_symbol, "timestamp")           |
| idx_annotation_linked_note_id                | CREATE INDEX idx_annotation_linked_note_id ON public.annotation USING btree (linked_note_id)                                |
| idx_annotation_strategy_id                   | CREATE INDEX idx_annotation_strategy_id ON public.annotation USING btree (strategy_id)                                      |
| idx_annotation_strategy_id_execution_step_id | CREATE INDEX idx_annotation_strategy_id_execution_step_id ON public.annotation USING btree (strategy_id, execution_step_id) |

## Relations

```mermaid
erDiagram

"public.annotation" }o--o| "public.strategy" : "FOREIGN KEY (strategy_id) REFERENCES strategy(id) ON DELETE CASCADE"
"public.annotation" }o--o| "public.note" : "FOREIGN KEY (linked_note_id) REFERENCES note(id) ON DELETE SET NULL"

"public.annotation" {
  uuid id
  uuid strategy_id FK
  varchar target_symbol
  varchar target_kind
  timestamp_with_time_zone timestamp
  numeric price
  text text
  varchar status
  uuid linked_note_id FK
  varchar created_by_kind
  timestamp_with_time_zone created_at
  timestamp_with_time_zone updated_at
  uuid execution_step_id
  text execution_task_id
}
"public.strategy" {
  uuid id
  varchar name
  text description
  integer sort_order
  timestamp_with_time_zone created_at
  timestamp_with_time_zone updated_at
}
"public.note" {
  uuid id
  uuid strategy_id FK
  varchar kind FK
  varchar trigger
  varchar trigger_label
  timestamp_with_time_zone created_at
  timestamp_with_time_zone updated_at
  text execution_id
}
```

---

> Generated by [tbls](https://github.com/k1LoW/tbls)
