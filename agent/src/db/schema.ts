import type { PushNotificationConfig, Task } from '@a2a-js/sdk'
import { sql } from 'drizzle-orm'
import {
  index,
  jsonb,
  pgTable,
  primaryKey,
  text,
  timestamp,
} from 'drizzle-orm/pg-core'

// Mirrors the A2A protocol's Task shape (v0.3) with the hot-path fields
// (context_id, state, status_timestamp) broken out into their own columns
// for indexing; `task` keeps the full object so a v1.0 SDK upgrade can be
// absorbed without a column migration.
export const a2aTasks = pgTable(
  'a2a_tasks',
  {
    taskId: text('task_id').primaryKey(),
    contextId: text('context_id').notNull(),
    state: text('state').notNull(),
    statusTimestamp: timestamp('status_timestamp', {
      withTimezone: true,
      mode: 'date',
    }).notNull(),
    protocolVersion: text('protocol_version').notNull().default('0.3'),
    task: jsonb('task').$type<Task>().notNull(),
    createdAt: timestamp('created_at', { withTimezone: true, mode: 'date' })
      .notNull()
      .default(sql`now()`),
  },
  (table) => [
    index('a2a_tasks_context_idx').on(table.contextId),
    index('a2a_tasks_sweep_idx').on(table.state, table.statusTimestamp),
  ],
)

// No FK to a2a_tasks: DefaultRequestHandler.sendMessage() saves the push
// config for a new task before the task row itself exists (the task is only
// inserted once the executor publishes its first event), so a FK here would
// reject every push-notification-enabled submission. Retention cleanup for
// orphaned rows is handled explicitly by the store instead of ON DELETE CASCADE.
export const a2aPushConfigs = pgTable(
  'a2a_push_configs',
  {
    taskId: text('task_id').notNull(),
    configId: text('config_id').notNull(),
    config: jsonb('config').$type<PushNotificationConfig>().notNull(),
    createdAt: timestamp('created_at', { withTimezone: true, mode: 'date' })
      .notNull()
      .default(sql`now()`),
  },
  (table) => [primaryKey({ columns: [table.taskId, table.configId] })],
)
