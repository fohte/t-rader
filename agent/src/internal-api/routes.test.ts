import type {
  Message,
  MessageSendParams,
  Task,
  TaskQueryParams,
} from '@a2a-js/sdk'
import type { A2ARequestHandler } from '@a2a-js/sdk/server'
import { A2AError } from '@a2a-js/sdk/server'
import { OpenAPIHono } from '@hono/zod-openapi'
import { describe, expect, it } from 'vitest'

import { mountInternalApiRoutes } from '#internal-api/routes'

const notImplemented = (): never => {
  throw new Error('not implemented in test stub')
}

const buildTask = (overrides: Partial<Task> & { id: string }): Task => ({
  contextId: `ctx-${overrides.id}`,
  kind: 'task',
  status: { state: 'submitted', timestamp: '2026-01-01T00:00:00.000Z' },
  ...overrides,
})

interface StubHandlerOverrides {
  sendMessage?: (params: MessageSendParams) => Promise<Message | Task>
  getTask?: (params: TaskQueryParams) => Promise<Task>
}

const buildStubHandler = (
  overrides: StubHandlerOverrides = {},
): A2ARequestHandler => ({
  getAgentCard: notImplemented,
  getAuthenticatedExtendedAgentCard: notImplemented,
  sendMessage: overrides.sendMessage ?? notImplemented,
  sendMessageStream: notImplemented,
  getTask: overrides.getTask ?? notImplemented,
  cancelTask: notImplemented,
  setTaskPushNotificationConfig: notImplemented,
  getTaskPushNotificationConfig: notImplemented,
  listTaskPushNotificationConfigs: notImplemented,
  deleteTaskPushNotificationConfig: notImplemented,
  resubscribe: notImplemented,
})

const buildApp = (requestHandler: A2ARequestHandler): OpenAPIHono => {
  const app = new OpenAPIHono()
  mountInternalApiRoutes(app, { requestHandler })
  return app
}

describe('POST /internal/tasks', () => {
  it('submits the prompt as an A2A message scoped by strategy_id and returns the task id', async () => {
    let capturedParams: MessageSendParams | undefined
    const app = buildApp(
      buildStubHandler({
        sendMessage: (params) => {
          capturedParams = params
          return Promise.resolve(buildTask({ id: 'task-1' }))
        },
      }),
    )

    const res = await app.request('/internal/tasks', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        strategy_id: '11111111-1111-1111-1111-111111111111',
        prompt: 'do the thing',
      }),
    })

    expect.soft(res.status).toBe(201)
    expect.soft(await res.json()).toEqual({ task_id: 'task-1' })
    expect.soft(capturedParams?.message.metadata).toEqual({
      strategy_id: '11111111-1111-1111-1111-111111111111',
    })
    expect.soft(capturedParams?.message.parts[0]).toEqual({
      kind: 'text',
      text: 'do the thing',
    })
    expect.soft(capturedParams?.configuration?.blocking).toBe(false)
  })

  it('returns 400 for a malformed JSON body', async () => {
    const app = buildApp(buildStubHandler())
    const res = await app.request('/internal/tasks', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: 'not json',
    })
    expect(res.status).toBe(400)
  })

  it('returns 422 when strategy_id is missing', async () => {
    const app = buildApp(buildStubHandler())
    const res = await app.request('/internal/tasks', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ prompt: 'do the thing' }),
    })
    expect(res.status).toBe(422)
  })

  it('returns 422 when prompt is missing', async () => {
    const app = buildApp(buildStubHandler())
    const res = await app.request('/internal/tasks', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        strategy_id: '11111111-1111-1111-1111-111111111111',
      }),
    })
    expect(res.status).toBe(422)
  })
})

describe('GET /internal/tasks/:taskId', () => {
  it('returns the task state for a submitted task', async () => {
    const app = buildApp(
      buildStubHandler({
        getTask: () =>
          Promise.resolve(
            buildTask({
              id: 'task-2',
              status: {
                state: 'working',
                timestamp: '2026-01-01T00:00:00.000Z',
              },
            }),
          ),
      }),
    )
    const res = await app.request('/internal/tasks/task-2')
    expect(res.status).toBe(200)
    expect(await res.json()).toEqual({ task_id: 'task-2', state: 'working' })
  })

  it('includes result_text for a completed task', async () => {
    const app = buildApp(
      buildStubHandler({
        getTask: () =>
          Promise.resolve(
            buildTask({
              id: 'task-3',
              status: {
                state: 'completed',
                timestamp: '2026-01-01T00:00:00.000Z',
                message: {
                  kind: 'message',
                  role: 'agent',
                  messageId: 'm1',
                  parts: [{ kind: 'text', text: 'the result' }],
                },
              },
            }),
          ),
      }),
    )
    const res = await app.request('/internal/tasks/task-3')
    expect(await res.json()).toEqual({
      task_id: 'task-3',
      state: 'completed',
      result_text: 'the result',
    })
  })

  it('includes error_kind for a failed task carrying usage_limit metadata', async () => {
    const app = buildApp(
      buildStubHandler({
        getTask: () =>
          Promise.resolve(
            buildTask({
              id: 'task-4',
              status: {
                state: 'failed',
                timestamp: '2026-01-01T00:00:00.000Z',
                message: {
                  kind: 'message',
                  role: 'agent',
                  messageId: 'm1',
                  parts: [{ kind: 'text', text: 'quota exceeded' }],
                  metadata: { error_kind: 'usage_limit' },
                },
              },
            }),
          ),
      }),
    )
    const res = await app.request('/internal/tasks/task-4')
    expect(await res.json()).toEqual({
      task_id: 'task-4',
      state: 'failed',
      error_kind: 'usage_limit',
    })
  })

  it('includes steps for a task carrying an agent-graph-steps artifact', async () => {
    const app = buildApp(
      buildStubHandler({
        getTask: () =>
          Promise.resolve(
            buildTask({
              id: 'task-5',
              status: {
                state: 'working',
                timestamp: '2026-01-01T00:00:00.000Z',
              },
              artifacts: [
                {
                  artifactId: 'agent-graph-steps',
                  name: 'agent-graph-steps',
                  parts: [
                    {
                      kind: 'data',
                      data: {
                        steps: [
                          {
                            phase_key: 'plan',
                            label: '調査計画',
                            model: 'claude-opus-4',
                            status: 'running',
                            started_at: '2026-01-01T00:00:00.000Z',
                            trace_id: 'trace-1',
                            span_id: 'span-1',
                          },
                        ],
                      },
                    },
                  ],
                },
              ],
            }),
          ),
      }),
    )
    const res = await app.request('/internal/tasks/task-5')
    expect(await res.json()).toEqual({
      task_id: 'task-5',
      state: 'working',
      steps: [
        {
          phase_key: 'plan',
          label: '調査計画',
          model: 'claude-opus-4',
          status: 'running',
          started_at: '2026-01-01T00:00:00.000Z',
          trace_id: 'trace-1',
          span_id: 'span-1',
        },
      ],
    })
  })

  it('returns 404 when the task does not exist', async () => {
    const app = buildApp(
      buildStubHandler({
        getTask: () => Promise.reject(A2AError.taskNotFound('missing-task')),
      }),
    )
    const res = await app.request('/internal/tasks/missing-task')
    expect(res.status).toBe(404)
    expect(await res.json()).toEqual({ error: 'task not found' })
  })
})
