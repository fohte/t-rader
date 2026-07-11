import { randomUUID } from 'node:crypto'

import type {
  Message,
  MessageSendParams,
  PushNotificationConfig,
  Task,
} from '@a2a-js/sdk'
import { A2AError } from '@a2a-js/sdk/server'
import type { A2ARequestHandler } from '@a2a-js/sdk/server'
import type { Hono } from 'hono'
import { z } from 'zod'

const TASK_NOT_FOUND_ERROR_CODE = -32001

const submitTaskBodySchema = z.object({
  strategy_id: z.string().min(1),
  prompt: z.string().min(1),
})

export interface InternalApiOptions {
  requestHandler: A2ARequestHandler
  // The backend's webhook to notify on task settlement. Registered as the
  // A2A push notification config for every submitted task so the SDK's own
  // push delivery (DefaultPushNotificationSender) fires it.
  pushNotificationConfig?: PushNotificationConfig
}

const buildUserMessage = (strategyId: string, prompt: string): Message => ({
  kind: 'message',
  role: 'user',
  messageId: randomUUID(),
  parts: [{ kind: 'text', text: prompt }],
  metadata: { strategy_id: strategyId },
})

const resultTextOf = (task: Task): string | undefined => {
  if (task.status.state !== 'completed') return undefined
  const part = task.status.message?.parts.find((p) => p.kind === 'text')
  return part?.kind === 'text' ? part.text : undefined
}

const errorKindOf = (task: Task): string | undefined => {
  const raw = task.status.message?.metadata?.['error_kind']
  return typeof raw === 'string' ? raw : undefined
}

const toTaskResponse = (
  task: Task,
): {
  task_id: string
  state: string
  result_text?: string
  error_kind?: string
} => {
  const resultText = resultTextOf(task)
  const errorKind = errorKindOf(task)
  return {
    task_id: task.id,
    state: task.status.state,
    ...(resultText !== undefined ? { result_text: resultText } : {}),
    ...(errorKind !== undefined ? { error_kind: errorKind } : {}),
  }
}

const isTaskNotFoundError = (err: unknown): boolean =>
  err instanceof A2AError && err.code === TASK_NOT_FOUND_ERROR_CODE

export const mountInternalApiRoutes = (
  app: Hono,
  options: InternalApiOptions,
): void => {
  const { requestHandler, pushNotificationConfig } = options

  app.post('/internal/tasks', async (c) => {
    let rawBody: unknown
    try {
      rawBody = await c.req.json()
    } catch {
      return c.json({ error: 'invalid JSON body' }, 400)
    }

    const parsed = submitTaskBodySchema.safeParse(rawBody)
    if (!parsed.success) {
      return c.json(
        { error: 'invalid request body', issues: parsed.error.issues },
        422,
      )
    }

    const message = buildUserMessage(
      parsed.data.strategy_id,
      parsed.data.prompt,
    )
    const params: MessageSendParams = {
      message,
      configuration: {
        blocking: false,
        ...(pushNotificationConfig !== undefined
          ? { pushNotificationConfig }
          : {}),
      },
    }
    const result = await requestHandler.sendMessage(params)
    const taskId = result.kind === 'task' ? result.id : result.taskId

    if (taskId === undefined) {
      return c.json({ error: 'agent did not create a task' }, 500)
    }
    return c.json({ task_id: taskId }, 201)
  })

  app.get('/internal/tasks/:taskId', async (c) => {
    const taskId = c.req.param('taskId')
    try {
      const task = await requestHandler.getTask({ id: taskId })
      return c.json(toTaskResponse(task))
    } catch (err) {
      if (isTaskNotFoundError(err)) {
        return c.json({ error: 'task not found' }, 404)
      }
      throw err
    }
  })
}
