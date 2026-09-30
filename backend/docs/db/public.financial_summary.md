# public.financial_summary

## Columns

| Name                           | Type             | Default | Nullable | Children | Parents | Comment |
| ------------------------------ | ---------------- | ------- | -------- | -------- | ------- | ------- |
| code                           | varchar          |         | false    |          |         |         |
| disclosure_no                  | varchar          |         | false    |          |         |         |
| disclosure_date                | date             |         | false    |          |         |         |
| report_group_key               | varchar          |         | false    |          |         |         |
| document_type                  | varchar          |         | true     |          |         |         |
| current_period_type            | varchar          |         | true     |          |         |         |
| current_period_start           | date             |         | true     |          |         |         |
| current_period_end             | date             |         | true     |          |         |         |
| current_fiscal_year_start      | date             |         | true     |          |         |         |
| current_fiscal_year_end        | date             |         | true     |          |         |         |
| sales                          | double precision |         | true     |          |         |         |
| operating_profit               | double precision |         | true     |          |         |         |
| ordinary_profit                | double precision |         | true     |          |         |         |
| net_profit                     | double precision |         | true     |          |         |         |
| eps                            | double precision |         | true     |          |         |         |
| bps                            | double precision |         | true     |          |         |         |
| total_assets                   | double precision |         | true     |          |         |         |
| equity                         | double precision |         | true     |          |         |         |
| equity_to_asset_ratio          | double precision |         | true     |          |         |         |
| roe                            | double precision |         | true     |          |         |         |
| cash_flow_operating            | double precision |         | true     |          |         |         |
| cash_flow_investing            | double precision |         | true     |          |         |         |
| cash_flow_financing            | double precision |         | true     |          |         |         |
| cash_and_equivalents           | double precision |         | true     |          |         |         |
| dividend_annual                | double precision |         | true     |          |         |         |
| dividend_annual_forecast       | double precision |         | true     |          |         |         |
| dividend_annual_forecast_next  | double precision |         | true     |          |         |         |
| forecast_sales                 | double precision |         | true     |          |         |         |
| forecast_operating_profit      | double precision |         | true     |          |         |         |
| forecast_ordinary_profit       | double precision |         | true     |          |         |         |
| forecast_net_profit            | double precision |         | true     |          |         |         |
| forecast_eps                   | double precision |         | true     |          |         |         |
| next_forecast_sales            | double precision |         | true     |          |         |         |
| next_forecast_operating_profit | double precision |         | true     |          |         |         |
| next_forecast_ordinary_profit  | double precision |         | true     |          |         |         |
| next_forecast_net_profit       | double precision |         | true     |          |         |         |
| next_forecast_eps              | double precision |         | true     |          |         |         |

## Constraints

| Name                   | Type        | Definition                        |
| ---------------------- | ----------- | --------------------------------- |
| financial_summary_pkey | PRIMARY KEY | PRIMARY KEY (code, disclosure_no) |

## Indexes

| Name                                  | Definition                                                                                                       |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| financial_summary_pkey                | CREATE UNIQUE INDEX financial_summary_pkey ON public.financial_summary USING btree (code, disclosure_no)         |
| idx_financial_summary_disclosure_date | CREATE INDEX idx_financial_summary_disclosure_date ON public.financial_summary USING btree (disclosure_date)     |
| idx_financial_summary_code_prefix     | CREATE INDEX idx_financial_summary_code_prefix ON public.financial_summary USING btree ("left"((code)::text, 4)) |

## Relations

```mermaid
erDiagram


"public.financial_summary" {
  varchar code
  varchar disclosure_no
  date disclosure_date
  varchar report_group_key
  varchar document_type
  varchar current_period_type
  date current_period_start
  date current_period_end
  date current_fiscal_year_start
  date current_fiscal_year_end
  double_precision sales
  double_precision operating_profit
  double_precision ordinary_profit
  double_precision net_profit
  double_precision eps
  double_precision bps
  double_precision total_assets
  double_precision equity
  double_precision equity_to_asset_ratio
  double_precision roe
  double_precision cash_flow_operating
  double_precision cash_flow_investing
  double_precision cash_flow_financing
  double_precision cash_and_equivalents
  double_precision dividend_annual
  double_precision dividend_annual_forecast
  double_precision dividend_annual_forecast_next
  double_precision forecast_sales
  double_precision forecast_operating_profit
  double_precision forecast_ordinary_profit
  double_precision forecast_net_profit
  double_precision forecast_eps
  double_precision next_forecast_sales
  double_precision next_forecast_operating_profit
  double_precision next_forecast_ordinary_profit
  double_precision next_forecast_net_profit
  double_precision next_forecast_eps
}
```

---

> Generated by [tbls](https://github.com/k1LoW/tbls)
