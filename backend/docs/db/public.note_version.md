# public.note_version

## Columns

| Name             | Type                     | Default           | Nullable | Children                                                                          | Parents                       | Comment |
| ---------------- | ------------------------ | ----------------- | -------- | --------------------------------------------------------------------------------- | ----------------------------- | ------- |
| id               | uuid                     | gen_random_uuid() | false    | [public.trade_note](public.trade_note.md) [public.note_link](public.note_link.md) |                               |         |
| note_id          | uuid                     |                   | false    |                                                                                   | [public.note](public.note.md) |         |
| version_no       | integer                  |                   | false    |                                                                                   |                               |         |
| title            | text                     |                   | false    |                                                                                   |                               |         |
| body_md          | text                     |                   | false    |                                                                                   |                               |         |
| frontmatter_json | jsonb                    | '{}'::jsonb       | false    |                                                                                   |                               |         |
| graphs_json      | jsonb                    | '[]'::jsonb       | false    |                                                                                   |                               |         |
| status           | text                     | 'unread'::text    | false    |                                                                                   |                               |         |
| is_current       | boolean                  | false             | false    |                                                                                   |                               |         |
| change_reason    | text                     |                   | true     |                                                                                   |                               |         |
| created_by_kind  | text                     |                   | false    |                                                                                   |                               |         |
| execution_id     | text                     |                   | true     |                                                                                   |                               |         |
| created_at       | timestamp with time zone | CURRENT_TIMESTAMP | false    |                                                                                   |                               |         |
| reviewed_at      | timestamp with time zone |                   | true     |                                                                                   |                               |         |

## Constraints

| Name                               | Type        | Definition                                                                         |
| ---------------------------------- | ----------- | ---------------------------------------------------------------------------------- |
| note_version_created_by_kind_check | CHECK       | CHECK ((created_by_kind = ANY (ARRAY['human'::text, 'llm'::text])))                |
| note_version_status_check          | CHECK       | CHECK ((status = ANY (ARRAY['approved'::text, 'unread'::text, 'rejected'::text]))) |
| note_version_note_id_fkey          | FOREIGN KEY | FOREIGN KEY (note_id) REFERENCES note(id) ON DELETE CASCADE                        |
| note_version_pkey                  | PRIMARY KEY | PRIMARY KEY (id)                                                                   |

## Indexes

| Name                                | Definition                                                                                                          |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| note_version_pkey                   | CREATE UNIQUE INDEX note_version_pkey ON public.note_version USING btree (id)                                       |
| note_version_note_id_version_no_key | CREATE UNIQUE INDEX note_version_note_id_version_no_key ON public.note_version USING btree (note_id, version_no)    |
| idx_note_version_current            | CREATE UNIQUE INDEX idx_note_version_current ON public.note_version USING btree (note_id) WHERE (is_current = true) |

## Relations

```mermaid
erDiagram

"public.trade_note" }o--|| "public.note_version" : "FOREIGN KEY (note_version_id) REFERENCES note_version(id) ON DELETE CASCADE"
"public.note_link" }o--|| "public.note_version" : "FOREIGN KEY (from_version_id) REFERENCES note_version(id) ON DELETE CASCADE"
"public.note_link" }o--o| "public.note_version" : "FOREIGN KEY (to_version_id) REFERENCES note_version(id) ON DELETE SET NULL"
"public.note_version" }o--|| "public.note" : "FOREIGN KEY (note_id) REFERENCES note(id) ON DELETE CASCADE"

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
}
"public.trade_note" {
  uuid trade_id FK
  uuid note_id FK
  timestamp_with_time_zone created_at
  uuid note_version_id FK
}
"public.note_link" {
  uuid from_version_id FK
  uuid to_note_id FK
  uuid to_version_id FK
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
