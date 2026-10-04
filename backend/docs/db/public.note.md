# public.note

## Description

ノートの識別情報と作成時の契機を保持する。本文は note_version に保存する。

## Columns

| Name          | Type                     | Default           | Nullable | Children                                                                                                                                                                                                                                                  | Parents                                 | Comment                                |
| ------------- | ------------------------ | ----------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- | -------------------------------------- |
| id            | uuid                     |                   | false    | [public.note_ref](public.note_ref.md) [public.annotation](public.annotation.md) [public.trade_note](public.trade_note.md) [public.prediction](public.prediction.md) [public.note_version](public.note_version.md) [public.note_link](public.note_link.md) |                                         |                                        |
| kind          | varchar                  |                   | true     |                                                                                                                                                                                                                                                           | [public.note_kind](public.note_kind.md) | note_kind で定義されたノート分類キー。 |
| trigger       | varchar                  |                   | true     |                                                                                                                                                                                                                                                           |                                         | ノートが作成された契機の種別。         |
| trigger_label | varchar                  |                   | true     |                                                                                                                                                                                                                                                           |                                         | ノート作成の契機に付けた表示ラベル。   |
| created_at    | timestamp with time zone | CURRENT_TIMESTAMP | false    |                                                                                                                                                                                                                                                           |                                         |                                        |
| updated_at    | timestamp with time zone | CURRENT_TIMESTAMP | false    |                                                                                                                                                                                                                                                           |                                         |                                        |
| execution_id  | text                     |                   | true     |                                                                                                                                                                                                                                                           |                                         | ノートを作成したタスク実行の識別子。   |

## Constraints

| Name                     | Type        | Definition                                                                                                                                                                                  |
| ------------------------ | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| note_kind_nonblank_check | CHECK       | CHECK (((kind IS NULL) OR ((kind)::text ~ '[^[:space:]]'::text)))                                                                                                                           |
| note_trigger_check       | CHECK       | CHECK (((trigger IS NULL) OR ((trigger)::text = ANY ((ARRAY['hook'::character varying, 'cron'::character varying, 'on-demand'::character varying, 'manual'::character varying])::text[])))) |
| note_pkey                | PRIMARY KEY | PRIMARY KEY (id)                                                                                                                                                                            |
| fk_note_kind             | FOREIGN KEY | FOREIGN KEY (kind) REFERENCES note_kind(key) ON DELETE RESTRICT                                                                                                                             |

## Indexes

| Name                  | Definition                                                                                                           |
| --------------------- | -------------------------------------------------------------------------------------------------------------------- |
| note_pkey             | CREATE UNIQUE INDEX note_pkey ON public.note USING btree (id)                                                        |
| idx_note_execution_id | CREATE UNIQUE INDEX idx_note_execution_id ON public.note USING btree (execution_id) WHERE (execution_id IS NOT NULL) |

## Relations

```mermaid
erDiagram

"public.note_ref" }o--|| "public.note" : "FOREIGN KEY (note_id) REFERENCES note(id) ON DELETE CASCADE"
"public.annotation" }o--o| "public.note" : "FOREIGN KEY (linked_note_id) REFERENCES note(id) ON DELETE SET NULL"
"public.trade_note" }o--|| "public.note" : "FOREIGN KEY (note_id) REFERENCES note(id) ON DELETE CASCADE"
"public.prediction" }o--o| "public.note" : "FOREIGN KEY (note_id) REFERENCES note(id) ON DELETE SET NULL"
"public.note_version" }o--|| "public.note" : "FOREIGN KEY (note_id) REFERENCES note(id) ON DELETE CASCADE"
"public.note_link" }o--|| "public.note" : "FOREIGN KEY (to_note_id) REFERENCES note(id) ON DELETE CASCADE"
"public.note" }o--o| "public.note_kind" : "FOREIGN KEY (kind) REFERENCES note_kind(key) ON DELETE RESTRICT"

"public.note" {
  uuid id
  varchar kind FK
  varchar trigger
  varchar trigger_label
  timestamp_with_time_zone created_at
  timestamp_with_time_zone updated_at
  text execution_id
}
"public.note_ref" {
  uuid note_id FK
  varchar ref_kind
  varchar ref_id
}
"public.annotation" {
  uuid id
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
"public.trade_note" {
  uuid trade_id FK
  uuid note_id FK
  timestamp_with_time_zone created_at
  uuid note_version_id FK
}
"public.prediction" {
  uuid prediction_id
  uuid strategy_id FK
  uuid note_id FK
  varchar target_stock_id FK
  varchar benchmark_stock_id FK
  text direction
  numeric probability
  date base_date
  date due_date
  timestamp_with_time_zone created_at
}
"public.note_version" {
  uuid id
  uuid note_id FK
  integer version_no
  text title
  text body_md
  jsonb frontmatter_json
  jsonb graphs_json
  text status
  boolean is_current
  text change_reason
  text created_by_kind
  text execution_id
  timestamp_with_time_zone created_at
  timestamp_with_time_zone reviewed_at
  jsonb resolved_price_references_json
}
"public.note_link" {
  uuid from_version_id FK
  uuid to_note_id FK
  uuid to_version_id FK
}
"public.note_kind" {
  text key
  text display_name
  boolean requires_approval
  text description
  integer sort_order
}
```

---

> Generated by [tbls](https://github.com/k1LoW/tbls)
