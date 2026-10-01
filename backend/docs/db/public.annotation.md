# public.annotation

## Columns

| Name              | Type                     | Default                     | Nullable | Children | Parents                               | Comment |
| ----------------- | ------------------------ | --------------------------- | -------- | -------- | ------------------------------------- | ------- |
| id                | uuid                     |                             | false    |          |                                       |         |
| strategy_id       | uuid                     |                             | true     |          | [public.strategy](public.strategy.md) |         |
| target_symbol     | varchar                  |                             | false    |          |                                       |         |
| target_kind       | varchar                  |                             | false    |          |                                       |         |
| timestamp         | timestamp with time zone |                             | false    |          |                                       |         |
| price             | numeric                  |                             | true     |          |                                       |         |
| text              | text                     |                             | false    |          |                                       |         |
| status            | varchar                  | 'unread'::character varying | false    |          |                                       |         |
| linked_note_id    | uuid                     |                             | true     |          | [public.note](public.note.md)         |         |
| created_by_kind   | varchar                  |                             | false    |          |                                       |         |
| created_at        | timestamp with time zone | CURRENT_TIMESTAMP           | false    |          |                                       |         |
| updated_at        | timestamp with time zone | CURRENT_TIMESTAMP           | false    |          |                                       |         |
| execution_step_id | uuid                     |                             | true     |          |                                       |         |
| execution_task_id | text                     |                             | true     |          |                                       |         |

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
