import {
  DefaultPushNotificationSender,
  DefaultRequestHandler,
} from '@a2a-js/sdk/server'
import type { Hono } from 'hono'
import { expect, it } from 'vitest'

import { buildAgentCard } from '@/a2a/agent-card'
import { TraderAgentExecutor } from '@/a2a/executor'
import { PostgresPushNotificationStore } from '@/a2a/postgres-push-notification-store'
import { PostgresTaskStore } from '@/a2a/postgres-task-store'
import { createApp } from '@/app'
import { describeIfDb, setupDrizzleTx } from '@/test/db'

interface TaskResponse {
  task_id: string
  state: string
  result_text?: string
  error_kind?: string
}

const sleep = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms))

const TERMINAL_STATES = new Set(['completed', 'failed', 'canceled', 'rejected'])

// DefaultRequestHandler fires the push notification as fire-and-forget (not
// awaited) on every event. It shares this test's reserved tx connection, so
// without a grace pause here it can still be querying that connection after
// this test's afterEach ROLLBACKs and releases it back to the pool — and the
// next test's BEGIN on the same recycled connection then races it, corrupting
// both. A real deployment doesn't recycle connections mid-request like this.
const settlePushNotification = (): Promise<void> => sleep(50)

// message/send with `blocking: false` returns as soon as the *first* event
// is available (the initial submitted Task), not once the executor
// finishes — mirrors how a real client (the backend watcher) has to poll
// tasks/get rather than assume the state has already settled by the time
// the submit response comes back.
const pollUntilTerminal = async (
  app: Hono,
  taskId: string,
): Promise<TaskResponse> => {
  for (let attempt = 0; attempt < 20; attempt++) {
    const res = await app.request(`/internal/tasks/${taskId}`, {
      headers: { authorization: 'Bearer test-token' },
    })
    const body = (await res.json()) as TaskResponse
    if (TERMINAL_STATES.has(body.state)) return body
    await sleep(10)
  }
  throw new Error(`task ${taskId} did not reach a terminal state in time`)
}

describeIfDb('t-rader-agent internal API integration', () => {
  const getTx = setupDrizzleTx()

  const buildApp = () => {
    const sql = getTx()
    const taskStore = new PostgresTaskStore(sql)
    const pushNotificationStore = new PostgresPushNotificationStore(sql)
    const pushNotificationSender = new DefaultPushNotificationSender(
      pushNotificationStore,
    )
    const agentCard = buildAgentCard({ url: 'http://localhost/' })
    const executor = new TraderAgentExecutor({ taskStore })
    const requestHandler = new DefaultRequestHandler(
      agentCard,
      taskStore,
      executor,
      undefined,
      pushNotificationStore,
      pushNotificationSender,
    )
    return createApp({
      sql,
      agentCard,
      requestHandler,
      internalApiToken: 'test-token',
      // Port 0 fails fast instead of hanging on a real connection attempt;
      // DefaultPushNotificationSender only logs the failure, it doesn't throw.
      backendPushNotificationConfig: {
        url: 'http://127.0.0.1:0/notify',
        token: 'wh-token',
      },
    })
  }

  it('submits a task with a valid strategy_id through the full A2A machinery to completion', async () => {
    const app = buildApp()

    const submitRes = await app.request('/internal/tasks', {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        authorization: 'Bearer test-token',
      },
      body: JSON.stringify({
        strategy_id: '11111111-1111-1111-1111-111111111111',
        prompt: 'do the thing',
      }),
    })
    const { task_id: taskId } = (await submitRes.json()) as { task_id: string }

    const finalState = await pollUntilTerminal(app, taskId)
    await settlePushNotification()

    expect({ submitStatus: submitRes.status, finalState }).toEqual({
      submitStatus: 201,
      finalState: {
        task_id: taskId,
        state: 'completed',
        result_text: 'strategy agent execution is not implemented yet',
      },
    })
  })

  it('rejects a task submitted with an invalid strategy_id', async () => {
    const app = buildApp()

    const submitRes = await app.request('/internal/tasks', {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        authorization: 'Bearer test-token',
      },
      body: JSON.stringify({
        strategy_id: 'not-a-uuid',
        prompt: 'do the thing',
      }),
    })
    const { task_id: taskId } = (await submitRes.json()) as { task_id: string }

    const finalState = await pollUntilTerminal(app, taskId)
    await settlePushNotification()

    expect(finalState).toEqual({ task_id: taskId, state: 'rejected' })
  })

  it('rejects a task submitted with no strategy_id at all', async () => {
    const app = buildApp()

    const submitRes = await app.request('/internal/tasks', {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        authorization: 'Bearer test-token',
      },
      body: JSON.stringify({ strategy_id: '', prompt: 'do the thing' }),
    })

    expect(submitRes.status).toBe(422)
  })

  it('returns 401 for an internal API request with a missing or wrong bearer token', async () => {
    const app = buildApp()

    const res = await app.request('/internal/tasks', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        strategy_id: '11111111-1111-1111-1111-111111111111',
        prompt: 'do the thing',
      }),
    })

    expect(res.status).toBe(401)
  })
})
