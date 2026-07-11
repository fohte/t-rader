import type {
  AgentCard,
  Message,
  MessageSendParams,
  Task,
  TaskArtifactUpdateEvent,
  TaskStatusUpdateEvent,
} from '@a2a-js/sdk'
import type { A2ARequestHandler } from '@a2a-js/sdk/server'
import { Hono } from 'hono'
import { describe, expect, it } from 'vitest'

import { mountA2aRoutes } from '@/a2a/hono-bridge'

const notImplemented = (): never => {
  throw new Error('not implemented in test stub')
}

const buildAgentCard = (): AgentCard => ({
  name: 'trader-agent',
  description: 'test agent',
  protocolVersion: '0.3',
  url: 'http://localhost/',
  version: '0.0.0',
  capabilities: { pushNotifications: true, streaming: true },
  defaultInputModes: ['text'],
  defaultOutputModes: ['text'],
  skills: [],
})

const buildTask = (id: string): Task => ({
  id,
  contextId: `ctx-${id}`,
  kind: 'task',
  status: { state: 'submitted', timestamp: '2026-01-01T00:00:00.000Z' },
})

interface StubHandlerOverrides {
  sendMessage?: (params: MessageSendParams) => Promise<Message | Task>
  sendMessageStream?: (
    params: MessageSendParams,
  ) => AsyncGenerator<
    Message | Task | TaskStatusUpdateEvent | TaskArtifactUpdateEvent,
    void,
    undefined
  >
}

const buildStubHandler = (
  overrides: StubHandlerOverrides = {},
): A2ARequestHandler => ({
  getAgentCard: () => Promise.resolve(buildAgentCard()),
  getAuthenticatedExtendedAgentCard: () => Promise.resolve(buildAgentCard()),
  sendMessage: overrides.sendMessage ?? notImplemented,
  sendMessageStream: overrides.sendMessageStream ?? notImplemented,
  getTask: () => notImplemented(),
  cancelTask: () => notImplemented(),
  setTaskPushNotificationConfig: notImplemented,
  getTaskPushNotificationConfig: notImplemented,
  listTaskPushNotificationConfigs: notImplemented,
  deleteTaskPushNotificationConfig: notImplemented,
  resubscribe: notImplemented,
})

const buildApp = (
  requestHandler: A2ARequestHandler,
  bearerToken?: string,
): Hono => {
  const app = new Hono()
  mountA2aRoutes(app, {
    agentCard: buildAgentCard(),
    requestHandler,
    ...(bearerToken !== undefined ? { bearerToken } : {}),
  })
  return app
}

describe('mountA2aRoutes', () => {
  it('serves the agent card unauthenticated', async () => {
    const app = buildApp(buildStubHandler(), 'secret')
    const res = await app.request('/.well-known/agent-card.json')

    expect(res.status).toBe(200)
    expect(await res.json()).toEqual(buildAgentCard())
  })

  it('forwards message/send to the request handler and returns its result', async () => {
    const task = buildTask('task-1')
    const app = buildApp(
      buildStubHandler({ sendMessage: () => Promise.resolve(task) }),
    )

    const res = await app.request('/', {
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

    expect(await res.json()).toEqual({ jsonrpc: '2.0', id: 1, result: task })
  })

  it('rejects a JSON-RPC POST without the configured bearer token', async () => {
    const app = buildApp(buildStubHandler(), 'secret')

    const res = await app.request('/', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        method: 'tasks/get',
        params: { id: 'x' },
      }),
    })

    expect(res.status).toBe(401)
  })

  it('accepts a JSON-RPC POST with a matching bearer token', async () => {
    const task = buildTask('task-2')
    const app = buildApp(
      buildStubHandler({ sendMessage: () => Promise.resolve(task) }),
      'secret',
    )

    const res = await app.request('/', {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        authorization: 'Bearer secret',
      },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 2,
        method: 'message/send',
        params: {
          message: {
            kind: 'message',
            role: 'user',
            messageId: 'm2',
            parts: [{ kind: 'text', text: 'hi' }],
          },
        },
      }),
    })

    expect(await res.json()).toEqual({ jsonrpc: '2.0', id: 2, result: task })
  })

  it('streams message/stream results over SSE', async () => {
    const task = buildTask('task-3')
    // eslint-disable-next-line @typescript-eslint/require-await -- must be an async generator to satisfy AsyncGenerator, even though it never awaits
    async function* stream(): AsyncGenerator<Task, void, undefined> {
      yield task
    }
    const app = buildApp(
      buildStubHandler({
        sendMessageStream: () => stream(),
      }),
    )

    const res = await app.request('/', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 3,
        method: 'message/stream',
        params: {
          message: {
            kind: 'message',
            role: 'user',
            messageId: 'm3',
            parts: [{ kind: 'text', text: 'hi' }],
          },
        },
      }),
    })

    const text = await res.text()
    expect(res.headers.get('content-type')).toBe('text/event-stream')
    expect(text).toBe(
      `data: ${JSON.stringify({ jsonrpc: '2.0', id: 3, result: task })}\n\n`,
    )
  })
})
