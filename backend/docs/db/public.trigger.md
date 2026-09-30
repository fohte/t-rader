# public.trigger

## Columns

| Name            | Type                     | Default           | Nullable | Children | Parents                                       | Comment |
| --------------- | ------------------------ | ----------------- | -------- | -------- | --------------------------------------------- | ------- |
| trigger_id      | uuid                     | gen_random_uuid() | false    |          |                                               |         |
| strategy_id     | uuid                     |                   | true     |          | [public.strategy](public.strategy.md)         |         |
| kind            | text                     |                   | false    |          |                                               |         |
| schedule        | text                     |                   | true     |          |                                               |         |
| hook_slug       | text                     |                   | true     |          |                                               |         |
| event_match     | jsonb                    |                   | true     |          |                                               |         |
| prompt_template | text                     |                   | false    |          |                                               |         |
| enabled         | boolean                  | true              | false    |          |                                               |         |
| last_fired_at   | timestamp with time zone |                   | true     |          |                                               |         |
| created_at      | timestamp with time zone | CURRENT_TIMESTAMP | false    |          |                                               |         |
| updated_at      | timestamp with time zone | CURRENT_TIMESTAMP | false    |          |                                               |         |
| purpose         | text                     |                   | true     |          | [public.agent_config](public.agent_config.md) |         |

## Constraints

| Name                            | Type        | Definition                                                                                                                                                         |
| ------------------------------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| trigger_kind_shape_check        | CHECK       | CHECK ((((kind = 'cron'::text) AND (schedule IS NOT NULL) AND (hook_slug IS NULL)) OR ((kind = 'hook'::text) AND (hook_slug IS NOT NULL) AND (schedule IS NULL)))) |
| trigger_strategy_id_fkey        | FOREIGN KEY | FOREIGN KEY (strategy_id) REFERENCES strategy(id) ON DELETE CASCADE                                                                                                |
| trigger_pkey                    | PRIMARY KEY | PRIMARY KEY (trigger_id)                                                                                                                                           |
| fk_trigger_purpose_agent_config | FOREIGN KEY | FOREIGN KEY (purpose) REFERENCES agent_config(purpose) ON DELETE SET NULL                                                                                          |

## Indexes

| Name                     | Definition                                                                                                               |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------ |
| trigger_pkey             | CREATE UNIQUE INDEX trigger_pkey ON public.trigger USING btree (trigger_id)                                              |
| trigger_hook_slug_idx    | CREATE UNIQUE INDEX trigger_hook_slug_idx ON public.trigger USING btree (hook_slug) WHERE (hook_slug IS NOT NULL)        |
| trigger_cron_enabled_idx | CREATE INDEX trigger_cron_enabled_idx ON public.trigger USING btree (enabled, last_fired_at) WHERE (kind = 'cron'::text) |
| trigger_strategy_idx     | CREATE INDEX trigger_strategy_idx ON public.trigger USING btree (strategy_id, created_at DESC)                           |

## Relations

```mermaid
erDiagram

"public.trigger" }o--o| "public.strategy" : "FOREIGN KEY (strategy_id) REFERENCES strategy(id) ON DELETE CASCADE"
"public.trigger" }o--o| "public.agent_config" : "FOREIGN KEY (purpose) REFERENCES agent_config(purpose) ON DELETE SET NULL"

"public.trigger" {
  uuid trigger_id
  uuid strategy_id FK
  text kind
  text schedule
  text hook_slug
  jsonb event_match
  text prompt_template
  boolean enabled
  timestamp_with_time_zone last_fired_at
  timestamp_with_time_zone created_at
  timestamp_with_time_zone updated_at
  text purpose FK
}
"public.strategy" {
  uuid id
  varchar name
  text description
  integer sort_order
  timestamp_with_time_zone created_at
  timestamp_with_time_zone updated_at
}
"public.agent_config" {
  uuid id
  text purpose
  text agents_md
  jsonb skills
  text agent_graph
  timestamp_with_time_zone created_at
  timestamp_with_time_zone updated_at
}
```

---

> Generated by [tbls](https://github.com/k1LoW/tbls)
