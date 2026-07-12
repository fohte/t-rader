import type { AgentCard, Task } from '@a2a-js/sdk'
import type { A2ARequestHandler } from '@a2a-js/sdk/server'
import { describe, expect, it } from 'vitest'

import { createApp } from '@/app'
import type { Sql } from '@/db'

const notImplemented = (): never => {
  throw new Error('not implemented in test stub')
}

const buildAgentCard = (): AgentCard => ({
  name: 'trader-agent',
  description: 'test agent',
  protocolVersion: '0.3',
  url: 'http://localhost/',
  version: '0.0.0',
  capabilities: { pushNotifications: true },
  defaultInputModes: ['text'],
  defaultOutputModes: ['text'],
  skills: [],
})

const buildStubHandler = (): A2ARequestHandler => ({
  getAgentCard: () => Promise.resolve(buildAgentCard()),
  getAuthenticatedExtendedAgentCard: notImplemented,
  sendMessage: () =>
    Promise.resolve({
      id: 'task-1',
      contextId: 'ctx-1',
      kind: 'task',
      status: { state: 'submitted', timestamp: '2026-01-01T00:00:00.000Z' },
    } satisfies Task),
  sendMessageStream: notImplemented,
  getTask: notImplemented,
  cancelTask: notImplemented,
  setTaskPushNotificationConfig: notImplemented,
  getTaskPushNotificationConfig: notImplemented,
  listTaskPushNotificationConfigs: notImplemented,
  deleteTaskPushNotificationConfig: notImplemented,
  resubscribe: notImplemented,
})

const fakeSql = (
  tag: (strings: TemplateStringsArray) => Promise<unknown[]>,
): Sql =>
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- Sql is a callable tag; tests only exercise the SELECT 1 ping branch.
  tag as unknown as Sql

const buildTestApp = (sql: Sql) =>
  createApp({
    sql,
    agentCard: buildAgentCard(),
    requestHandler: buildStubHandler(),
    internalApiToken: 'internal-secret',
    backendPushNotificationConfig: {
      url: 'http://backend/notify',
      token: 'wh-token',
    },
  })

describe('createApp', () => {
  it('returns ok on /health without touching the DB', async () => {
    const queries: string[][] = []
    const sql = fakeSql((strings) => {
      queries.push(Array.from(strings))
      return Promise.resolve([])
    })

    const res = await buildTestApp(sql).request('/health')
    expect(res.status).toBe(200)
    expect(await res.json()).toEqual({ status: 'ok' })
    expect(queries).toEqual([])
  })

  it('returns ok on /health/ready after a successful DB ping', async () => {
    const queries: string[][] = []
    const sql = fakeSql((strings) => {
      queries.push(Array.from(strings))
      return Promise.resolve([])
    })

    const res = await buildTestApp(sql).request('/health/ready')
    expect(res.status).toBe(200)
    expect(await res.json()).toEqual({ status: 'ok' })
    expect(queries).toEqual([['SELECT 1']])
  })

  it('returns 503 on /health/ready when the DB ping fails', async () => {
    const sql = fakeSql(() => Promise.reject(new Error('connection refused')))
    const res = await buildTestApp(sql).request('/health/ready')
    expect(res.status).toBe(503)
    expect(await res.json()).toEqual({
      status: 'error',
      error: 'connection refused',
    })
  })

  it('serves the agent card without authentication', async () => {
    const sql = fakeSql(() => Promise.resolve([]))
    const res = await buildTestApp(sql).request('/.well-known/agent-card.json')
    expect(res.status).toBe(200)
  })

  it('rejects /internal/tasks without the internal API bearer token', async () => {
    const sql = fakeSql(() => Promise.resolve([]))
    const res = await buildTestApp(sql).request('/internal/tasks', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ strategy_id: '1', prompt: 'hi' }),
    })
    expect(res.status).toBe(401)
  })

  it('accepts /internal/tasks with the internal API bearer token', async () => {
    const sql = fakeSql(() => Promise.resolve([]))
    const res = await buildTestApp(sql).request('/internal/tasks', {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        authorization: 'Bearer internal-secret',
      },
      body: JSON.stringify({
        strategy_id: '11111111-1111-1111-1111-111111111111',
        prompt: 'hi',
      }),
    })
    expect(res.status).toBe(201)
    expect(await res.json()).toEqual({ task_id: 'task-1' })
  })

  it('forwards A2A JSON-RPC requests to the request handler', async () => {
    const sql = fakeSql(() => Promise.resolve([]))
    const res = await buildTestApp(sql).request('/', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        method: 'message/send',
        params: {
          message: {
            kind: 'message',
            role: 'user',
            messageId: 'm1',
            parts: [{ kind: 'text', text: 'hi' }],
          },
        },
      }),
    })
    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the JSON-RPC result's id, not the full response schema
    const body = (await res.json()) as { result?: { id?: string } }
    expect(body.result?.id).toBe('task-1')
  })
})
