import { randomUUID } from 'node:crypto'

import type { Message, Task, TaskStatusUpdateEvent } from '@a2a-js/sdk'
import type {
  AgentExecutor,
  ExecutionEventBus,
  RequestContext,
  TaskStore,
} from '@a2a-js/sdk/server'

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

export const extractStrategyId = (message: Message): string | undefined => {
  const raw = message.metadata?.['strategy_id']
  return typeof raw === 'string' ? raw : undefined
}

const isValidStrategyId = (value: string | undefined): value is string =>
  value !== undefined && UUID_RE.test(value)

const buildAgentMessage = (
  text: string,
  taskId: string,
  contextId: string,
): Message => ({
  kind: 'message',
  role: 'agent',
  messageId: randomUUID(),
  taskId,
  contextId,
  parts: [{ kind: 'text', text }],
})

export interface TraderAgentExecutorDeps {
  taskStore: Pick<TaskStore, 'load'>
}

// Validates strategy_id and drives the task through the A2A lifecycle.
// Actual strategy execution (AgentConfigApi lookup, LangGraph agent
// construction, MCP tool calls) is not implemented yet — this always
// completes with a placeholder result once strategy_id passes validation.
export class TraderAgentExecutor implements AgentExecutor {
  constructor(private readonly deps: TraderAgentExecutorDeps) {}

  async execute(
    requestContext: RequestContext,
    eventBus: ExecutionEventBus,
  ): Promise<void> {
    const { taskId, contextId, userMessage } = requestContext
    const strategyId = extractStrategyId(userMessage)

    if (!isValidStrategyId(strategyId)) {
      const rejectedTask: Task = {
        id: taskId,
        contextId,
        kind: 'task',
        history: [userMessage],
        status: {
          state: 'rejected',
          timestamp: new Date().toISOString(),
          message: buildAgentMessage(
            'strategy_id is missing or not a valid UUID',
            taskId,
            contextId,
          ),
        },
      }
      eventBus.publish(rejectedTask)
      eventBus.publish({
        kind: 'status-update',
        taskId,
        contextId,
        final: true,
        status: rejectedTask.status,
      } satisfies TaskStatusUpdateEvent)
      eventBus.finished()
      return
    }

    const submittedTask: Task = {
      id: taskId,
      contextId,
      kind: 'task',
      history: [userMessage],
      status: { state: 'submitted', timestamp: new Date().toISOString() },
    }
    eventBus.publish(submittedTask)

    eventBus.publish({
      kind: 'status-update',
      taskId,
      contextId,
      final: false,
      status: { state: 'working', timestamp: new Date().toISOString() },
    } satisfies TaskStatusUpdateEvent)

    const resultText = await this.runStrategyAgent(strategyId, userMessage)

    eventBus.publish({
      kind: 'status-update',
      taskId,
      contextId,
      final: true,
      status: {
        state: 'completed',
        timestamp: new Date().toISOString(),
        message: buildAgentMessage(resultText, taskId, contextId),
      },
    } satisfies TaskStatusUpdateEvent)
    eventBus.finished()
  }

  async cancelTask(taskId: string, eventBus: ExecutionEventBus): Promise<void> {
    const task = await this.deps.taskStore.load(taskId)
    eventBus.publish({
      kind: 'status-update',
      taskId,
      contextId: task?.contextId ?? '',
      final: true,
      status: { state: 'canceled', timestamp: new Date().toISOString() },
    } satisfies TaskStatusUpdateEvent)
    eventBus.finished()
  }

  private runStrategyAgent(
    _strategyId: string,
    _userMessage: Message,
  ): Promise<string> {
    return Promise.resolve('strategy agent execution is not implemented yet')
  }
}
