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
import { GenAiCallbackHandler } from '@/genai/genai-callback-handler'
import {
  createStrategyAgentDeps,
  runStrategyAgent,
} from '@/strategy-agent/strategy-agent'
import { createStrategyCandidatesFetcher } from '@/strategy-resolution/mgmt-mcp-client'

const GEN_AI_PROVIDER_NAME = 'opencode'

// Upper bound on graceful shutdown: server.close()'s callback only fires once
// every open connection ends, so a client holding a keep-alive connection
// open would otherwise hang the process indefinitely.
const SHUTDOWN_FORCE_EXIT_MS = 10_000

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
  const strategyAgentDeps = createStrategyAgentDeps({
    backendApiBaseUrl: env.BACKEND_API_BASE_URL,
    strategyMcpUrl: env.STRATEGY_MCP_URL,
    openCodeApiKey: env.OPENCODE_API_KEY,
    genAiCallbackHandler: new GenAiCallbackHandler({
      providerName: GEN_AI_PROVIDER_NAME,
    }),
  })
  const executor = new TraderAgentExecutor({
    taskStore,
    runStrategyAgent: (strategyId, userMessage) =>
      runStrategyAgent(strategyAgentDeps, strategyId, userMessage),
    fetchStrategyCandidates: createStrategyCandidatesFetcher(env.MGMT_MCP_URL),
  })
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
    const forceExit = setTimeout(() => {
      console.error('shutdown timed out, forcing exit')
      process.exit(1)
    }, SHUTDOWN_FORCE_EXIT_MS)

    server.close((closeErr) => {
      clearTimeout(forceExit)
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
