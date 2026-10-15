# public.trigger

## Description

時刻、外部 hook、システム内イベントを契機にエージェント task を起動する設定。

## Columns

| Name               | Type                     | Default           | Nullable | Children                                      | Parents                                       | Comment                                                                                                |
| ------------------ | ------------------------ | ----------------- | -------- | --------------------------------------------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| trigger_id         | uuid                     | gen_random_uuid() | false    | [public.event_firing](public.event_firing.md) |                                               | trigger を識別する ID。                                                                                |
| strategy_id        | uuid                     |                   | true     |                                               | [public.strategy](public.strategy.md)         | タスクの実行対象となる戦略。                                                                           |
| kind               | text                     |                   | false    |                                               |                                               | 起動方式。                                                                                             |
| schedule           | text                     |                   | true     |                                               |                                               | cron 起動に使う UTC のスケジュール式。                                                                 |
| hook_slug          | text                     |                   | true     |                                               |                                               | hook 起動時に trigger を識別するパス名。                                                               |
| event_match        | jsonb                    |                   | true     |                                               |                                               | payload が起動条件を満たすか判定する条件。                                                             |
| prompt_template    | text                     |                   | false    |                                               |                                               | 起動時にエージェントへ渡す指示文のテンプレート。                                                       |
| enabled            | boolean                  | true              | false    |                                               |                                               | trigger が有効かどうか。                                                                               |
| last_fired_at      | timestamp with time zone |                   | true     |                                               |                                               | 最後に trigger が発火した時刻。                                                                        |
| created_at         | timestamp with time zone | CURRENT_TIMESTAMP | false    |                                               |                                               |                                                                                                        |
| updated_at         | timestamp with time zone | CURRENT_TIMESTAMP | false    |                                               |                                               |                                                                                                        |
| purpose            | text                     |                   | true     |                                               | [public.agent_config](public.agent_config.md) | 実行に使用する agent_config の purpose キー。                                                          |
| business_days_only | boolean                  | false             | false    |                                               |                                               | 土日・日本の祝日・年末年始を休場日として扱い、臨時休場日は判定せずに cron trigger の起動日を絞る設定。 |
| event_type         | text                     |                   | true     |                                               |                                               | kind=event の trigger が受け取るイベントの種類。                                                       |

## Constraints

| Name                            | Type        | Definition                                                                                                                                                                                                                                                                                                                                                         |
| ------------------------------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| trigger_kind_shape_check        | CHECK       | CHECK ((((kind = 'cron'::text) AND (schedule IS NOT NULL) AND (hook_slug IS NULL) AND (event_type IS NULL)) OR ((kind = 'hook'::text) AND (hook_slug IS NOT NULL) AND (schedule IS NULL) AND (event_type IS NULL)) OR ((kind = 'event'::text) AND (event_type ~ '^[a-z][a-z0-9_]_([.][a-z][a-z0-9\_]_)+$'::text) AND (schedule IS NULL) AND (hook_slug IS NULL)))) |
| trigger_strategy_id_fkey        | FOREIGN KEY | FOREIGN KEY (strategy_id) REFERENCES strategy(id) ON DELETE CASCADE                                                                                                                                                                                                                                                                                                |
| trigger_pkey                    | PRIMARY KEY | PRIMARY KEY (trigger_id)                                                                                                                                                                                                                                                                                                                                           |
| fk_trigger_purpose_agent_config | FOREIGN KEY | FOREIGN KEY (purpose) REFERENCES agent_config(purpose) ON DELETE SET NULL                                                                                                                                                                                                                                                                                          |

## Indexes

| Name                      | Definition                                                                                                                   |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| trigger_pkey              | CREATE UNIQUE INDEX trigger_pkey ON public.trigger USING btree (trigger_id)                                                  |
| trigger_hook_slug_idx     | CREATE UNIQUE INDEX trigger_hook_slug_idx ON public.trigger USING btree (hook_slug) WHERE (hook_slug IS NOT NULL)            |
| trigger_cron_enabled_idx  | CREATE INDEX trigger_cron_enabled_idx ON public.trigger USING btree (enabled, last_fired_at) WHERE (kind = 'cron'::text)     |
| trigger_strategy_idx      | CREATE INDEX trigger_strategy_idx ON public.trigger USING btree (strategy_id, created_at DESC)                               |
| trigger_event_enabled_idx | CREATE INDEX trigger_event_enabled_idx ON public.trigger USING btree (event_type) WHERE ((kind = 'event'::text) AND enabled) |

## Relations

```mermaid
erDiagram

"public.event_firing" }o--|| "public.trigger" : "FOREIGN KEY (trigger_id) REFERENCES trigger(trigger_id) ON DELETE CASCADE"
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
  boolean business_days_only
  text event_type
}
"public.event_firing" {
  uuid event_id FK
  uuid trigger_id FK
  uuid task_id FK
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
