import { randomUUID } from 'node:crypto'

import type {
  Message,
  MessageSendParams,
  PushNotificationConfig,
  Task,
} from '@a2a-js/sdk'
import type { A2ARequestHandler } from '@a2a-js/sdk/server'
import { A2AError } from '@a2a-js/sdk/server'
import { createRoute, OpenAPIHono, z } from '@hono/zod-openapi'
import { ResultAsync } from 'neverthrow'

import { AGENT_GRAPH_STEPS_ARTIFACT_ID } from '#strategy-agent/agent-graph/step'

const TASK_NOT_FOUND_ERROR_CODE = -32001

const submitTaskBodySchema = z.object({
  strategy_id: z.string().min(1),
  prompt: z.string().min(1),
})

const submitTaskResponseSchema = z.object({
  task_id: z.string(),
})

const taskIdParamsSchema = z.object({
  taskId: z.string(),
})

// steps の要素の中身は producer 側 (StrategyTaskStep) の型で保証されている前提
// とし、backend 同様に検証しない (backend/src/agent_client/client.rs の
// GetTaskResponse.steps も serde_json::Value で素通ししている)。
const taskResponseSchema = z.object({
  task_id: z.string(),
  state: z.string(),
  result_text: z.string().optional(),
  error_kind: z.string().optional(),
  steps: z.array(z.unknown()).optional(),
})

const errorResponseSchema = z.object({
  error: z.string(),
})

const validationErrorResponseSchema = z.object({
  error: z.string(),
  issues: z.array(z.unknown()),
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

// agent-graph-steps artifact (executor.ts が artifact-update で都度置換して
// いる) から steps 配列を取り出す。producer 側 (StrategyTaskStep) の型で
// 形は保証されている前提とし、要素の中身までは検証しない。
const stepsOf = (task: Task): unknown[] | undefined => {
  const artifact = task.artifacts?.find(
    (a) => a.artifactId === AGENT_GRAPH_STEPS_ARTIFACT_ID,
  )
  const part = artifact?.parts.find((p) => p.kind === 'data')
  const steps = part?.kind === 'data' ? part.data['steps'] : undefined
  return Array.isArray(steps) ? steps : undefined
}

const toTaskResponse = (
  task: Task,
): {
  task_id: string
  state: string
  result_text?: string
  error_kind?: string
  steps?: unknown[]
} => {
  const resultText = resultTextOf(task)
  const errorKind = errorKindOf(task)
  const steps = stepsOf(task)
  return {
    task_id: task.id,
    state: task.status.state,
    ...(resultText !== undefined ? { result_text: resultText } : {}),
    ...(errorKind !== undefined ? { error_kind: errorKind } : {}),
    ...(steps !== undefined ? { steps } : {}),
  }
}

const isTaskNotFoundError = (err: unknown): boolean =>
  err instanceof A2AError && err.code === TASK_NOT_FOUND_ERROR_CODE

const submitTaskRoute = createRoute({
  method: 'post',
  path: '/internal/tasks',
  request: {
    body: {
      content: { 'application/json': { schema: submitTaskBodySchema } },
    },
  },
  responses: {
    201: {
      content: { 'application/json': { schema: submitTaskResponseSchema } },
      description: 'Task submitted',
    },
    400: {
      content: { 'application/json': { schema: errorResponseSchema } },
      description: 'Malformed JSON body',
    },
    422: {
      content: {
        'application/json': { schema: validationErrorResponseSchema },
      },
      description: 'Request body failed validation',
    },
    500: {
      content: { 'application/json': { schema: errorResponseSchema } },
      description: 'Agent did not create a task',
    },
  },
})

const getTaskRoute = createRoute({
  method: 'get',
  path: '/internal/tasks/{taskId}',
  request: {
    params: taskIdParamsSchema,
  },
  responses: {
    200: {
      content: { 'application/json': { schema: taskResponseSchema } },
      description: 'Task status',
    },
    404: {
      content: { 'application/json': { schema: errorResponseSchema } },
      description: 'Task not found',
    },
  },
})

export const mountInternalApiRoutes = (
  app: OpenAPIHono,
  options: InternalApiOptions,
): void => {
  const { requestHandler, pushNotificationConfig } = options

  app.openapi(
    submitTaskRoute,
    async (c) => {
      const { strategy_id, prompt } = c.req.valid('json')
      const message = buildUserMessage(strategy_id, prompt)
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
    },
    (result, c) => {
      if (!result.success) {
        return c.json(
          { error: 'invalid request body', issues: result.error.issues },
          422,
        )
      }
      return undefined
    },
  )

  app.openapi(getTaskRoute, async (c) => {
    const { taskId } = c.req.valid('param')
    const taskResult = await ResultAsync.fromPromise(
      requestHandler.getTask({ id: taskId }),
      (err) => err,
    )
    if (taskResult.isErr()) {
      if (isTaskNotFoundError(taskResult.error)) {
        return c.json({ error: 'task not found' }, 404)
      }
      // Not task-not-found: rethrown so app.onError's catch-all handles
      // logging/Sentry/500 the same as any other unexpected failure.
      // eslint-disable-next-line no-restricted-syntax -- 上記の通り、タスク未検出以外は再送出
      throw taskResult.error
    }
    return c.json(toTaskResponse(taskResult.value), 200)
  })
}
