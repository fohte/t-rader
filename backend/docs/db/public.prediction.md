# public.prediction

## Description

戦略に記録した、対象銘柄と比較銘柄の将来リターンに関する予測。

## Columns

| Name               | Type                     | Default           | Nullable | Children                                              | Parents                               | Comment                                      |
| ------------------ | ------------------------ | ----------------- | -------- | ----------------------------------------------------- | ------------------------------------- | -------------------------------------------- |
| prediction_id      | uuid                     |                   | false    | [public.prediction_grade](public.prediction_grade.md) |                                       | 予測を識別する ID。                          |
| strategy_id        | uuid                     |                   | false    |                                                       | [public.strategy](public.strategy.md) | 予測を所有する戦略。                         |
| note_id            | uuid                     |                   | true     |                                                       | [public.note](public.note.md)         | 予測の根拠として関連付けたノート。           |
| target_stock_id    | varchar                  |                   | false    |                                                       | [public.stock](public.stock.md)       | 比較対象となる銘柄の ID。                    |
| benchmark_stock_id | varchar                  |                   | false    |                                                       | [public.stock](public.stock.md)       | リターンを比較する銘柄の ID。                |
| direction          | text                     |                   | false    |                                                       |                                       | 対象銘柄が比較銘柄を上回るか下回るかの予測。 |
| probability        | numeric                  |                   | false    |                                                       |                                       | 予測した方向が実現する確率。                 |
| base_date          | date                     |                   | false    |                                                       |                                       | 比較の起点とする終値の日付。                 |
| due_date           | date                     |                   | false    |                                                       |                                       | 比較の終点とする終値の日付。                 |
| created_at         | timestamp with time zone | CURRENT_TIMESTAMP | false    |                                                       |                                       |                                              |

## Constraints

| Name                               | Type        | Definition                                                                      |
| ---------------------------------- | ----------- | ------------------------------------------------------------------------------- |
| prediction_check                   | CHECK       | CHECK (((target_stock_id)::text <> (benchmark_stock_id)::text))                 |
| prediction_check1                  | CHECK       | CHECK ((due_date > base_date))                                                  |
| prediction_direction_check         | CHECK       | CHECK ((direction = ANY (ARRAY['outperform'::text, 'underperform'::text])))     |
| prediction_probability_check       | CHECK       | CHECK ((probability = ANY (ARRAY[0.55, 0.6, 0.65, 0.7, 0.75, 0.8, 0.85, 0.9]))) |
| prediction_strategy_id_fkey        | FOREIGN KEY | FOREIGN KEY (strategy_id) REFERENCES strategy(id) ON DELETE CASCADE             |
| prediction_benchmark_stock_id_fkey | FOREIGN KEY | FOREIGN KEY (benchmark_stock_id) REFERENCES stock(id)                           |
| prediction_target_stock_id_fkey    | FOREIGN KEY | FOREIGN KEY (target_stock_id) REFERENCES stock(id)                              |
| prediction_note_id_fkey            | FOREIGN KEY | FOREIGN KEY (note_id) REFERENCES note(id) ON DELETE SET NULL                    |
| prediction_pkey                    | PRIMARY KEY | PRIMARY KEY (prediction_id)                                                     |

## Indexes

| Name                        | Definition                                                                                        |
| --------------------------- | ------------------------------------------------------------------------------------------------- |
| prediction_pkey             | CREATE UNIQUE INDEX prediction_pkey ON public.prediction USING btree (prediction_id)              |
| idx_prediction_strategy_due | CREATE INDEX idx_prediction_strategy_due ON public.prediction USING btree (strategy_id, due_date) |

## Relations

```mermaid
erDiagram

"public.prediction_grade" |o--|| "public.prediction" : "FOREIGN KEY (prediction_id) REFERENCES prediction(prediction_id) ON DELETE CASCADE"
"public.prediction" }o--|| "public.strategy" : "FOREIGN KEY (strategy_id) REFERENCES strategy(id) ON DELETE CASCADE"
"public.prediction" }o--o| "public.note" : "FOREIGN KEY (note_id) REFERENCES note(id) ON DELETE SET NULL"
"public.prediction" }o--|| "public.stock" : "FOREIGN KEY (target_stock_id) REFERENCES stock(id)"
"public.prediction" }o--|| "public.stock" : "FOREIGN KEY (benchmark_stock_id) REFERENCES stock(id)"

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
"public.prediction_grade" {
  uuid prediction_id FK
  numeric target_base_close
  numeric target_due_close
  numeric benchmark_base_close
  numeric benchmark_due_close
  numeric target_return
  numeric benchmark_return
  boolean correct
  timestamp_with_time_zone graded_at
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
"public.stock" {
  varchar id
  varchar name
  varchar market
  varchar sector_id FK
  timestamp_with_time_zone created_at
  timestamp_with_time_zone updated_at
  varchar product_category
}
```

---

> Generated by [tbls](https://github.com/k1LoW/tbls)
