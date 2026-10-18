# public.us_stock_sip_bar_refetch_progress

## Description

米国株の SIP 足を再取得した銘柄と、再取得できない理由を保持する。

## Columns

| Name              | Type                     | Default | Nullable | Children | Parents                                     | Comment                                    |
| ----------------- | ------------------------ | ------- | -------- | -------- | ------------------------------------------- | ------------------------------------------ |
| instrument_id     | text                     |         | false    |          | [public.instruments](public.instruments.md) | SIP 足を再取得する銘柄。                   |
| completed_at      | timestamp with time zone |         | true     |          |                                             | 指定期間の足を SIP で再取得した時刻。      |
| skipped_reason    | text                     |         | true     |          |                                             | SIP で再取得できず、対象から除外した理由。 |
| resume_timeframe  | text                     |         | true     |          |                                             | 次回再開する時間足。                       |
| resume_from       | timestamp with time zone |         | true     |          |                                             | 再開する時間足の取得開始時刻。             |
| resume_to         | timestamp with time zone |         | true     |          |                                             | 再開する時間足の取得終了時刻。             |
| resume_page_token | text                     |         | true     |          |                                             | 次回再開する時間足の API ページトークン。  |

## Constraints

| Name                                                     | Type        | Definition                                                                                                   |
| -------------------------------------------------------- | ----------- | ------------------------------------------------------------------------------------------------------------ |
| us_stock_sip_bar_refetch_progress_check                  | CHECK       | CHECK (((completed_at IS NOT NULL) OR (skipped_reason IS NOT NULL) OR (resume_timeframe IS NOT NULL)))       |
| us_stock_sip_bar_refetch_progress_check1                 | CHECK       | CHECK (((completed_at IS NULL) OR (skipped_reason IS NULL)))                                                 |
| us_stock_sip_bar_refetch_progress_check2                 | CHECK       | CHECK (((resume_page_token IS NULL) OR (resume_timeframe IS NOT NULL)))                                      |
| us_stock_sip_bar_refetch_progress_check3                 | CHECK       | CHECK (((resume_timeframe IS NULL) OR (completed_at IS NULL)))                                               |
| us_stock_sip_bar_refetch_progress_check4                 | CHECK       | CHECK (((resume_timeframe IS NULL) OR (skipped_reason IS NULL)))                                             |
| us_stock_sip_bar_refetch_progress_check5                 | CHECK       | CHECK (((resume_timeframe IS NULL) OR (resume_from IS NOT NULL)))                                            |
| us_stock_sip_bar_refetch_progress_check6                 | CHECK       | CHECK (((resume_timeframe IS NULL) OR (resume_to IS NOT NULL)))                                              |
| us_stock_sip_bar_refetch_progress_check7                 | CHECK       | CHECK (((resume_from IS NULL) OR (resume_timeframe IS NOT NULL)))                                            |
| us_stock_sip_bar_refetch_progress_check8                 | CHECK       | CHECK (((resume_to IS NULL) OR (resume_timeframe IS NOT NULL)))                                              |
| us_stock_sip_bar_refetch_progress_resume_timeframe_check | CHECK       | CHECK (((resume_timeframe IS NULL) OR (resume_timeframe = ANY (ARRAY['1d'::text, '1h'::text, '1m'::text])))) |
| us_stock_sip_bar_refetch_progress_instrument_id_fkey     | FOREIGN KEY | FOREIGN KEY (instrument_id) REFERENCES instruments(id) ON DELETE CASCADE                                     |
| us_stock_sip_bar_refetch_progress_pkey                   | PRIMARY KEY | PRIMARY KEY (instrument_id)                                                                                  |

## Indexes

| Name                                   | Definition                                                                                                                         |
| -------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| us_stock_sip_bar_refetch_progress_pkey | CREATE UNIQUE INDEX us_stock_sip_bar_refetch_progress_pkey ON public.us_stock_sip_bar_refetch_progress USING btree (instrument_id) |

## Relations

```mermaid
erDiagram

"public.us_stock_sip_bar_refetch_progress" |o--|| "public.instruments" : "FOREIGN KEY (instrument_id) REFERENCES instruments(id) ON DELETE CASCADE"

"public.us_stock_sip_bar_refetch_progress" {
  text instrument_id FK
  timestamp_with_time_zone completed_at
  text skipped_reason
  text resume_timeframe
  timestamp_with_time_zone resume_from
  timestamp_with_time_zone resume_to
  text resume_page_token
}
"public.instruments" {
  varchar id
  varchar name
  varchar market
  varchar sector
}
```

---

> Generated by [tbls](https://github.com/k1LoW/tbls)
