import type { AgentCard, PushNotificationConfig } from '@a2a-js/sdk'
import type { A2ARequestHandler } from '@a2a-js/sdk/server'
import { captureWithFingerprint } from '@fohte/service-kit/observability'
import { OpenAPIHono } from '@hono/zod-openapi'
import { HTTPException } from 'hono/http-exception'
import type { BlankEnv } from 'hono/types'
import { ResultAsync } from 'neverthrow'

import { mountA2aRoutes } from '#a2a/hono-bridge'
import type { Sql } from '#db'
import { pingDb } from '#db'
import { bearerAuth } from '#internal-api/auth'
import { mountInternalApiRoutes } from '#internal-api/routes'

const REQUEST_FAILED_FINGERPRINT = 'app.request-failed'

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

export const createApp = (deps: AppDeps): OpenAPIHono<BlankEnv> => {
  const app = new OpenAPIHono<BlankEnv>()

  // Aggregated catch-all: an unexpected throw from any route (rather than
  // one already converted to a JSON response, like the health checks and
  // A2AError handling below) lands here exactly once. HTTPException carries
  // its own status (e.g. the zod-openapi body validator's 400 on malformed
  // JSON), which a flat 500 would otherwise discard.
  app.onError((err, c) => {
    if (err instanceof HTTPException) {
      return c.json({ error: err.message }, err.status)
    }
    console.error('request failed:', err)
    captureWithFingerprint(err, REQUEST_FAILED_FINGERPRINT, {
      extras: { path: c.req.path, method: c.req.method },
    })
    return c.json({ error: errorMessage(err) }, 500)
  })

  // liveness/startup probe 用
  app.get('/health', (c) => c.json({ status: 'ok' }))

  // readiness probe 用
  app.get('/health/ready', async (c) => {
    const pingResult = await ResultAsync.fromPromise(
      pingDb(deps.sql),
      (err) => err,
    )
    return pingResult.match(
      () => c.json({ status: 'ok' }),
      (err) => c.json({ status: 'error', error: errorMessage(err) }, 503),
    )
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
