import {
  DefaultPushNotificationSender,
  DefaultRequestHandler,
} from '@a2a-js/sdk/server'
import { serve } from '@hono/node-server'

import { buildAgentCard } from '@/a2a/agent-card'
import { TraderAgentExecutor } from '@/a2a/executor'
import { startTaskLifecycleJobs } from '@/a2a/lifecycle'
import { PostgresPushNotificationStore } from '@/a2a/postgres-push-notification-store'
import { PostgresTaskStore } from '@/a2a/postgres-task-store'
import { createApp } from '@/app'
import { observability } from '@/bootstrap'
import { createSql, pingDb } from '@/db'
import { runMigrations } from '@/db/migrations'
import { loadEnv } from '@/env'

export const main = async (): Promise<void> => {
  const env = loadEnv()
  const sql = createSql(env.DATABASE_URL)
  await pingDb(sql)
  await runMigrations(sql)

  const taskStore = new PostgresTaskStore(sql)
  const pushNotificationStore = new PostgresPushNotificationStore(sql)
  const pushNotificationSender = new DefaultPushNotificationSender(
    pushNotificationStore,
  )
  const agentCard = buildAgentCard({ url: env.TRADER_AGENT_URL })
  const executor = new TraderAgentExecutor({ taskStore })
  const requestHandler = new DefaultRequestHandler(
    agentCard,
    taskStore,
    executor,
    undefined,
    pushNotificationStore,
    pushNotificationSender,
  )

  const app = createApp({
    sql,
    agentCard,
    requestHandler,
    internalApiToken: env.INTERNAL_API_TOKEN,
    backendPushNotificationConfig: {
      url: env.BACKEND_WEBHOOK_URL,
      token: env.BACKEND_WEBHOOK_TOKEN,
    },
  })

  const lifecycleJobs = startTaskLifecycleJobs(taskStore, {
    workingTimeoutMs: env.A2A_WATCHDOG_TIMEOUT_MS,
    retentionDays: env.A2A_RETENTION_DAYS,
    onExpire: (task) => pushNotificationSender.send(task),
  })

  const server = serve(
    { fetch: app.fetch, port: env.TRADER_AGENT_PORT, hostname: '0.0.0.0' },
    (info) => {
      console.log(
        `t-rader-agent listening on ${info.address}:${String(info.port)}`,
      )
    },
  )

  const shutdown = (signal: NodeJS.Signals): void => {
    console.log(`received ${signal}, shutting down`)
    server.close((closeErr) => {
      void Promise.allSettled([
        lifecycleJobs.stop(),
        sql.end({ timeout: 5 }),
        observability?.shutdown(),
      ])
        .then((results) => {
          for (const result of results) {
            if (result.status === 'rejected') {
              console.error('shutdown error:', result.reason)
            }
          }
        })
        .finally(() => {
          process.exit(closeErr ? 1 : 0)
        })
    })
  }

  process.once('SIGTERM', shutdown)
  process.once('SIGINT', shutdown)
}
