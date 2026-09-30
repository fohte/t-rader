# public.strategy_task_step

## Columns

| Name              | Type                      | Default                                         | Nullable | Children | Parents                                         | Comment |
| ----------------- | ------------------------- | ----------------------------------------------- | -------- | -------- | ----------------------------------------------- | ------- |
| execution_step_id | uuid                      |                                                 | false    |          |                                                 |         |
| task_id           | uuid                      |                                                 | false    |          | [public.strategy_task](public.strategy_task.md) |         |
| phase_key         | text                      |                                                 | false    |          |                                                 |         |
| label             | text                      |                                                 | false    |          |                                                 |         |
| model             | text                      |                                                 | false    |          |                                                 |         |
| status            | strategy_task_step_status |                                                 | false    |          |                                                 |         |
| item              | jsonb                     |                                                 | true     |          |                                                 |         |
| item_label        | text                      |                                                 | true     |          |                                                 |         |
| output            | jsonb                     |                                                 | true     |          |                                                 |         |
| started_at        | timestamp with time zone  |                                                 | false    |          |                                                 |         |
| finished_at       | timestamp with time zone  |                                                 | true     |          |                                                 |         |
| trace_id          | text                      |                                                 | false    |          |                                                 |         |
| span_id           | text                      |                                                 | false    |          |                                                 |         |
| error             | text                      |                                                 | true     |          |                                                 |         |
| seq               | bigint                    | nextval('strategy_task_step_seq_seq'::regclass) | false    |          |                                                 |         |

## Constraints

| Name                            | Type        | Definition                                                                |
| ------------------------------- | ----------- | ------------------------------------------------------------------------- |
| strategy_task_step_task_id_fkey | FOREIGN KEY | FOREIGN KEY (task_id) REFERENCES strategy_task(task_id) ON DELETE CASCADE |
| strategy_task_step_pkey         | PRIMARY KEY | PRIMARY KEY (execution_step_id)                                           |

## Indexes

| Name                            | Definition                                                                                               |
| ------------------------------- | -------------------------------------------------------------------------------------------------------- |
| strategy_task_step_pkey         | CREATE UNIQUE INDEX strategy_task_step_pkey ON public.strategy_task_step USING btree (execution_step_id) |
| strategy_task_step_task_seq_idx | CREATE INDEX strategy_task_step_task_seq_idx ON public.strategy_task_step USING btree (task_id, seq)     |

## Relations

```mermaid
erDiagram

"public.strategy_task_step" }o--|| "public.strategy_task" : "FOREIGN KEY (task_id) REFERENCES strategy_task(task_id) ON DELETE CASCADE"

"public.strategy_task_step" {
  uuid execution_step_id
  uuid task_id FK
  text phase_key
  text label
  text model
  strategy_task_step_status status
  jsonb item
  text item_label
  jsonb output
  timestamp_with_time_zone started_at
  timestamp_with_time_zone finished_at
  text trace_id
  text span_id
  text error
  bigint seq
}
"public.strategy_task" {
  uuid task_id
  uuid strategy_id FK
  text a2a_task_id
  text source
  text prompt
  strategy_task_phase phase
  text error_summary
  timestamp_with_time_zone created_at
  timestamp_with_time_zone updated_at
  text result_text
  timestamp_with_time_zone deadline_at
  text purpose
  timestamp_with_time_zone as_of
  timestamp_with_time_zone auto_resumed_at
}
```

---

> Generated by [tbls](https://github.com/k1LoW/tbls)
