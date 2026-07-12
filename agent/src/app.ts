import type { AgentCard, PushNotificationConfig } from '@a2a-js/sdk'
import type { A2ARequestHandler } from '@a2a-js/sdk/server'
import { Hono } from 'hono'

import { mountA2aRoutes } from '@/a2a/hono-bridge'
import type { Sql } from '@/db'
import { pingDb } from '@/db'
import { bearerAuth } from '@/internal-api/auth'
import { mountInternalApiRoutes } from '@/internal-api/routes'

export interface AppDeps {
  sql: Sql
  agentCard: AgentCard
  requestHandler: A2ARequestHandler
  internalApiToken: string
  backendPushNotificationConfig: PushNotificationConfig
  // Cluster-internal auth for the A2A JSON-RPC surface (agent-to-agent
  // callers). Left unset, that surface is unauthenticated.
  a2aBearerToken?: string
}

const errorMessage = (err: unknown): string =>
  err instanceof Error ? err.message : String(err)

export const createApp = (deps: AppDeps): Hono => {
  const app = new Hono()

  // shallow health check: プロセスの生存確認のみ (liveness/startup probe 用)
  app.get('/health', (c) => c.json({ status: 'ok' }))

  // deep health check: DB 疎通まで検証する (readiness probe 用)
  app.get('/health/ready', async (c) => {
    try {
      await pingDb(deps.sql)
      return c.json({ status: 'ok' })
    } catch (err) {
      return c.json({ status: 'error', error: errorMessage(err) }, 503)
    }
  })

  app.use('/internal/*', bearerAuth(deps.internalApiToken))
  mountInternalApiRoutes(app, {
    requestHandler: deps.requestHandler,
    pushNotificationConfig: deps.backendPushNotificationConfig,
  })

  mountA2aRoutes(app, {
    agentCard: deps.agentCard,
    requestHandler: deps.requestHandler,
    ...(deps.a2aBearerToken !== undefined
      ? { bearerToken: deps.a2aBearerToken }
      : {}),
  })

  return app
}
