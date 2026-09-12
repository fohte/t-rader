import {
  DefaultPushNotificationSender,
  DefaultRequestHandler,
} from '@a2a-js/sdk/server'
import { captureWithFingerprint } from '@fohte/service-kit/observability'
import { serve } from '@hono/node-server'

import { buildAgentCard } from '#a2a/agent-card'
import { TraderAgentExecutor } from '#a2a/executor'
import { startTaskLifecycleJobs } from '#a2a/lifecycle'
import { PostgresPushNotificationStore } from '#a2a/postgres-push-notification-store'
import { PostgresTaskStore } from '#a2a/postgres-task-store'
import { createApp } from '#app'
import { observability } from '#bootstrap'
import { createSql, pingDb } from '#db'
import { runMigrations } from '#db/migrations'
import { loadEnv } from '#env'
import {
  createStrategyAgentDeps,
  runStrategyAgent,
} from '#strategy-agent/strategy-agent'
import { createStrategyCandidatesFetcher } from '#strategy-resolution/mgmt-mcp-client'

const GEN_AI_PROVIDER_NAME = 'opencode'

// Kept under Kubernetes' terminationGracePeriodSeconds (30s) with margin for
// the Sentry flush below. Doesn't guarantee an in-progress phase finishes in time.
const SHUTDOWN_FORCE_EXIT_MS = 20_000

const SHUTDOWN_TIMED_OUT_FINGERPRINT = 'main.shutdown-timed-out'

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
    llmApiKey: env.LLM_API_KEY,
    llmBaseUrl: env.LLM_BASE_URL,
    genAiProviderName: GEN_AI_PROVIDER_NAME,
  })
  const executor = new TraderAgentExecutor({
    taskStore,
    runStrategyAgent: (
      strategyId,
      purpose,
      taskId,
      userMessage,
      resumeSteps,
      onStepsChanged,
    ) =>
      runStrategyAgent(
        strategyAgentDeps,
        strategyId,
        purpose,
        taskId,
        userMessage,
        resumeSteps,
        onStepsChanged,
      ),
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

  let isShuttingDown = false

  const app = createApp({
    sql,
    agentCard,
    requestHandler,
    internalApiToken: env.INTERNAL_API_TOKEN,
    backendPushNotificationConfig: {
      url: env.BACKEND_WEBHOOK_URL,
      token: env.BACKEND_WEBHOOK_TOKEN,
    },
    isShuttingDown: () => isShuttingDown,
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
    isShuttingDown = true

    const forceExit = setTimeout(() => {
      console.error(
        'graceful shutdown timed out, forcing exit (in-flight task(s) may be abandoned)',
      )
      captureWithFingerprint(
        new Error('graceful shutdown timed out waiting for in-flight tasks'),
        SHUTDOWN_TIMED_OUT_FINGERPRINT,
      )
      // 直前の captureWithFingerprint はイベントをキューに積むだけなので、
      // 送信を待たずに exit すると Sentry に届く前にプロセスが消える。
      void (observability?.shutdown() ?? Promise.resolve()).finally(() => {
        process.exit(1)
      })
    }, SHUTDOWN_FORCE_EXIT_MS)

    server.close((closeErr) => {
      // executor.execute() の実行本体は HTTP コネクションに紐付かない
      // 切り離された Promise なので、server.close() を待つだけでは保護されない。
      void executor
        .waitForInFlightExecutions()
        .then(() =>
          Promise.allSettled([
            lifecycleJobs.stop(),
            sql.end({ timeout: 5 }),
            observability?.shutdown(),
          ]),
        )
        .then((results) => {
          for (const result of results) {
            if (result.status === 'rejected') {
              console.error('shutdown error:', result.reason)
            }
          }
        })
        .finally(() => {
          clearTimeout(forceExit)
          process.exit(closeErr ? 1 : 0)
        })
    })
  }

  process.once('SIGTERM', shutdown)
  process.once('SIGINT', shutdown)
}
