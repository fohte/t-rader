# public.instruments

## Description

価格データの取得対象となる金融商品を管理する。

## Columns

| Name   | Type    | Default | Nullable | Children                                                                                                                                                                                                                                                                              | Parents | Comment                                             |
| ------ | ------- | ------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- | --------------------------------------------------- |
| id     | varchar |         | false    | [public.bars](public.bars.md) [public.us_stock_hourly_bar_ingest_progress](public.us_stock_hourly_bar_ingest_progress.md) [public.bars_timestamp_coverage](public.bars_timestamp_coverage.md) [public.us_stock_sip_bar_refetch_progress](public.us_stock_sip_bar_refetch_progress.md) |         |                                                     |
| name   | varchar |         | false    |                                                                                                                                                                                                                                                                                       |         | 金融商品の名称。                                    |
| market | varchar |         | false    |                                                                                                                                                                                                                                                                                       |         | 金融商品が上場する市場。TSE、US、OTHER のいずれか。 |
| sector | varchar |         | true     |                                                                                                                                                                                                                                                                                       |         | 金融商品に設定された業種名。                        |

## Constraints

| Name                        | Type        | Definition                                                                                                                      |
| --------------------------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------- |
| instruments_id_not_null     | n           | NOT NULL id                                                                                                                     |
| instruments_market_check    | CHECK       | CHECK (((market)::text = ANY ((ARRAY['TSE'::character varying, 'US'::character varying, 'OTHER'::character varying])::text[]))) |
| instruments_market_not_null | n           | NOT NULL market                                                                                                                 |
| instruments_name_not_null   | n           | NOT NULL name                                                                                                                   |
| instruments_pkey            | PRIMARY KEY | PRIMARY KEY (id)                                                                                                                |

## Indexes

| Name             | Definition                                                                  |
| ---------------- | --------------------------------------------------------------------------- |
| instruments_pkey | CREATE UNIQUE INDEX instruments_pkey ON public.instruments USING btree (id) |

## Relations

```mermaid
erDiagram

"public.bars" }o--|| "public.instruments" : "FOREIGN KEY (instrument_id) REFERENCES instruments(id) ON DELETE CASCADE"
"public.us_stock_hourly_bar_ingest_progress" |o--|| "public.instruments" : "FOREIGN KEY (instrument_id) REFERENCES instruments(id) ON DELETE CASCADE"
"public.bars_timestamp_coverage" }o--|| "public.instruments" : "FOREIGN KEY (instrument_id) REFERENCES instruments(id) ON DELETE CASCADE"
"public.us_stock_sip_bar_refetch_progress" |o--|| "public.instruments" : "FOREIGN KEY (instrument_id) REFERENCES instruments(id) ON DELETE CASCADE"

"public.instruments" {
  varchar id
  varchar name
  varchar market
  varchar sector
}
"public.bars" {
  varchar instrument_id FK
  varchar timeframe
  timestamp_with_time_zone timestamp
  numeric open
  numeric high
  numeric low
  numeric close
  bigint volume
  numeric adjustment_factor
}
"public.us_stock_hourly_bar_ingest_progress" {
  text instrument_id FK
  timestamp_with_time_zone completed_through
}
"public.bars_timestamp_coverage" {
  text instrument_id FK
  text timeframe
  timestamp_with_time_zone earliest_timestamp
  timestamp_with_time_zone latest_timestamp
}
"public.us_stock_sip_bar_refetch_progress" {
  text instrument_id FK
  timestamp_with_time_zone completed_at
  text skipped_reason
  text resume_timeframe
  timestamp_with_time_zone resume_from
  timestamp_with_time_zone resume_to
  text resume_page_token
}
```

---

> Generated by [tbls](https://github.com/k1LoW/tbls)
