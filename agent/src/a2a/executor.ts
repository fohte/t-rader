import { randomUUID } from 'node:crypto'

import type { Message, Task, TaskStatusUpdateEvent } from '@a2a-js/sdk'
import type {
  AgentExecutor,
  ExecutionEventBus,
  RequestContext,
  TaskStore,
} from '@a2a-js/sdk/server'

import type { StrategyAgentResult } from '@/strategy-agent/strategy-agent'

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
  errorKind?: string,
): Message => ({
  kind: 'message',
  role: 'agent',
  messageId: randomUUID(),
  taskId,
  contextId,
  parts: [{ kind: 'text', text }],
  ...(errorKind !== undefined ? { metadata: { error_kind: errorKind } } : {}),
})

export interface TraderAgentExecutorDeps {
  taskStore: Pick<TaskStore, 'load'>
  runStrategyAgent: (
    strategyId: string,
    userMessage: Message,
  ) => Promise<StrategyAgentResult>
}

// Validates strategy_id and drives the task through the A2A lifecycle,
// delegating actual execution (AgentConfigApi lookup, LangGraph agent
// construction, MCP tool calls) to the injected runStrategyAgent.
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

    // runStrategyAgent maps its own known failure modes to a StrategyAgentResult,
    // but an unexpected rejection (e.g. MCP client construction throwing before
    // its own try/catch) must still resolve the task rather than leave it stuck
    // in working state with eventBus.finished() never called.
    try {
      const result = await this.deps.runStrategyAgent(strategyId, userMessage)
      eventBus.publish({
        kind: 'status-update',
        taskId,
        contextId,
        final: true,
        status: {
          state: result.status,
          timestamp: new Date().toISOString(),
          message: buildAgentMessage(
            result.message,
            taskId,
            contextId,
            result.errorKind,
          ),
        },
      } satisfies TaskStatusUpdateEvent)
    } catch (error) {
      eventBus.publish({
        kind: 'status-update',
        taskId,
        contextId,
        final: true,
        status: {
          state: 'failed',
          timestamp: new Date().toISOString(),
          message: buildAgentMessage(
            error instanceof Error ? error.message : String(error),
            taskId,
            contextId,
            'agent_error',
          ),
        },
      } satisfies TaskStatusUpdateEvent)
    } finally {
      eventBus.finished()
    }
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
}
