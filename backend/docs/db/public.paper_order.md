# public.paper_order

## Description

仮想口座に対して記録した不変の売買注文を保持する。

## Columns

| Name            | Type                     | Default           | Nullable | Children                                                  | Parents                                         | Comment                                    |
| --------------- | ------------------------ | ----------------- | -------- | --------------------------------------------------------- | ----------------------------------------------- | ------------------------------------------ |
| id              | uuid                     | gen_random_uuid() | false    | [public.paper_order_result](public.paper_order_result.md) |                                                 |                                            |
| account_id      | uuid                     |                   | false    |                                                           | [public.paper_account](public.paper_account.md) | 注文を記録した仮想口座。                   |
| stock_id        | varchar                  |                   | false    |                                                           | [public.stock](public.stock.md)                 | 注文対象の日本株。                         |
| side            | text                     |                   | false    |                                                           |                                                 | 買い注文または売り注文。                   |
| qty             | bigint                   |                   | false    |                                                           |                                                 | 注文数量。                                 |
| note_version_id | uuid                     |                   | false    |                                                           | [public.note_version](public.note_version.md)   | 注文の根拠として記録したノートバージョン。 |
| ordered_at      | timestamp with time zone | CURRENT_TIMESTAMP | false    |                                                           |                                                 | 注文を記録した時刻。                       |

## Constraints

| Name                             | Type        | Definition                                                              |
| -------------------------------- | ----------- | ----------------------------------------------------------------------- |
| paper_order_qty_check            | CHECK       | CHECK (((qty > 0) AND ((qty % (100)::bigint) = 0)))                     |
| paper_order_side_check           | CHECK       | CHECK ((side = ANY (ARRAY['buy'::text, 'sell'::text])))                 |
| paper_order_stock_id_fkey        | FOREIGN KEY | FOREIGN KEY (stock_id) REFERENCES stock(id)                             |
| paper_order_note_version_id_fkey | FOREIGN KEY | FOREIGN KEY (note_version_id) REFERENCES note_version(id)               |
| paper_order_account_id_fkey      | FOREIGN KEY | FOREIGN KEY (account_id) REFERENCES paper_account(id) ON DELETE CASCADE |
| paper_order_pkey                 | PRIMARY KEY | PRIMARY KEY (id)                                                        |

## Indexes

| Name                               | Definition                                                                                                 |
| ---------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| paper_order_pkey                   | CREATE UNIQUE INDEX paper_order_pkey ON public.paper_order USING btree (id)                                |
| idx_paper_order_account_ordered_at | CREATE INDEX idx_paper_order_account_ordered_at ON public.paper_order USING btree (account_id, ordered_at) |
| idx_paper_order_stock_id           | CREATE INDEX idx_paper_order_stock_id ON public.paper_order USING btree (stock_id)                         |
| idx_paper_order_note_version_id    | CREATE INDEX idx_paper_order_note_version_id ON public.paper_order USING btree (note_version_id)           |

## Relations

```mermaid
erDiagram

"public.paper_order_result" |o--|| "public.paper_order" : "FOREIGN KEY (order_id) REFERENCES paper_order(id) ON DELETE CASCADE"
"public.paper_order" }o--|| "public.paper_account" : "FOREIGN KEY (account_id) REFERENCES paper_account(id) ON DELETE CASCADE"
"public.paper_order" }o--|| "public.stock" : "FOREIGN KEY (stock_id) REFERENCES stock(id)"
"public.paper_order" }o--|| "public.note_version" : "FOREIGN KEY (note_version_id) REFERENCES note_version(id)"

"public.paper_order" {
  uuid id
  uuid account_id FK
  varchar stock_id FK
  text side
  bigint qty
  uuid note_version_id FK
  timestamp_with_time_zone ordered_at
}
"public.paper_order_result" {
  uuid order_id FK
  text outcome
  date fill_date
  numeric fill_price
  text reject_reason
  timestamp_with_time_zone decided_at
}
"public.paper_account" {
  uuid id
  text name
  uuid strategy_id FK
  text purpose FK
  numeric initial_cash_jpy
  varchar benchmark_stock_id FK
  date started_on
  timestamp_with_time_zone created_at
  timestamp_with_time_zone updated_at
}
"public.stock" {
  varchar id
  varchar name
  varchar market
  timestamp_with_time_zone created_at
  timestamp_with_time_zone updated_at
  varchar product_category
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
```

---

> Generated by [tbls](https://github.com/k1LoW/tbls)
