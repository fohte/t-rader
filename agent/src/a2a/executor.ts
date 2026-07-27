import { randomUUID } from 'node:crypto'

import type { Message, Task, TaskStatusUpdateEvent } from '@a2a-js/sdk'
import type {
  AgentExecutor,
  ExecutionEventBus,
  RequestContext,
  TaskStore,
} from '@a2a-js/sdk/server'
import { captureWithFingerprint } from '@fohte/service-kit/observability'

import { extractMessageText } from '#a2a/message-text'
import type { StrategyAgentResult } from '#strategy-agent/strategy-agent'
import type { FetchStrategyCandidates } from '#strategy-resolution/mgmt-mcp-client'
import type {
  StrategyCandidate,
  StrategyResolution,
} from '#strategy-resolution/resolve-strategy'
import { resolveStrategy } from '#strategy-resolution/resolve-strategy'

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

const STRATEGY_RESOLUTION_FAILED_FINGERPRINT =
  'a2a.executor.strategy-resolution-failed'
const TURN_FAILED_FINGERPRINT = 'a2a.executor.turn-failed'

export const extractStrategyId = (message: Message): string | undefined => {
  const raw = message.metadata?.['strategy_id']
  return typeof raw === 'string' ? raw : undefined
}

const isValidStrategyId = (value: string): boolean => UUID_RE.test(value)

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

// Only user-authored text feeds strategy resolution and the eventual
// analysis prompt. Including the agent's own clarifying question (which
// necessarily names the candidates it's asking about) would make every
// candidate it just listed match again on the next turn, defeating
// disambiguation.
const userConversationText = (messages: readonly Message[]): string =>
  messages
    .filter((m) => m.role === 'user')
    .map(extractMessageText)
    .join('\n')

const buildPromptMessage = (
  text: string,
  taskId: string,
  contextId: string,
): Message => ({
  kind: 'message',
  role: 'user',
  messageId: randomUUID(),
  taskId,
  contextId,
  parts: [{ kind: 'text', text }],
})

const describeCandidates = (candidates: readonly StrategyCandidate[]): string =>
  candidates.map((c) => c.name).join(', ')

const buildClarifyingMessageText = (
  resolution: Extract<StrategyResolution, { kind: 'ambiguous' | 'not_found' }>,
): string => {
  if (resolution.kind === 'ambiguous') {
    return `対象の戦略を一意に特定できませんでした。次のうちどれですか: ${describeCandidates(resolution.candidates)}`
  }
  if (resolution.candidates.length === 0) {
    return '利用可能な戦略が登録されていません。'
  }
  return `対象の戦略が見つかりませんでした。次のいずれかの戦略名を含めて教えてください: ${describeCandidates(resolution.candidates)}`
}

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

export interface TraderAgentExecutorDeps {
  taskStore: Pick<TaskStore, 'load'>
  runStrategyAgent: (
    strategyId: string,
    userMessage: Message,
  ) => Promise<StrategyAgentResult>
  // Looks up the current strategy list (via the backend's management MCP)
  // to resolve a strategy_id from free text when the caller doesn't supply
  // one via message metadata.
  fetchStrategyCandidates: FetchStrategyCandidates
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
    const { taskId, contextId, userMessage, task } = requestContext
    const rawStrategyId = extractStrategyId(userMessage)

    if (rawStrategyId !== undefined && !isValidStrategyId(rawStrategyId)) {
      const rejectedStatus = {
        state: 'rejected' as const,
        timestamp: new Date().toISOString(),
        message: buildAgentMessage(
          'strategy_id is missing or not a valid UUID',
          taskId,
          contextId,
        ),
      }
      // Same task === undefined guard as the submitted/working path below:
      // a resumed task's history was already updated by the framework, and
      // publishing a full Task event here would replace it wholesale.
      if (task === undefined) {
        eventBus.publish({
          id: taskId,
          contextId,
          kind: 'task',
          history: [userMessage],
          status: rejectedStatus,
        } satisfies Task)
      }
      eventBus.publish({
        kind: 'status-update',
        taskId,
        contextId,
        final: true,
        status: rejectedStatus,
      } satisfies TaskStatusUpdateEvent)
      eventBus.finished()
      return
    }

    // A brand-new task has no row in the store yet, so it needs a full Task
    // event to seed one. A resumed task (e.g. from input-required) already
    // has a row whose history the framework updated with this turn's
    // incoming message before execute() was called, so only a status-update
    // is published here to avoid clobbering that history.
    if (task === undefined) {
      eventBus.publish({
        id: taskId,
        contextId,
        kind: 'task',
        history: [userMessage],
        status: { state: 'submitted', timestamp: new Date().toISOString() },
      } satisfies Task)
    }
    eventBus.publish({
      kind: 'status-update',
      taskId,
      contextId,
      final: false,
      status: { state: 'working', timestamp: new Date().toISOString() },
    } satisfies TaskStatusUpdateEvent)

    let strategyId: string
    let promptMessage: Message
    if (rawStrategyId !== undefined) {
      strategyId = rawStrategyId
      promptMessage = userMessage
    } else {
      // Resolution matches against only the latest turn's text: on a
      // resumed task, that's the user's disambiguating reply (e.g. "長期の
      // 方で"), which names the intended strategy far more directly than
      // the accumulated conversation. Folding in the original (already
      // ambiguous) request would just dilute or re-tie the match.
      const latestText = extractMessageText(userMessage)

      const candidatesResult = await this.deps.fetchStrategyCandidates()
      if (candidatesResult.isErr()) {
        captureWithFingerprint(
          candidatesResult.error,
          STRATEGY_RESOLUTION_FAILED_FINGERPRINT,
          { extras: { taskId, contextId } },
        )
        eventBus.publish({
          kind: 'status-update',
          taskId,
          contextId,
          final: true,
          status: {
            state: 'failed',
            timestamp: new Date().toISOString(),
            message: buildAgentMessage(
              errorMessage(candidatesResult.error),
              taskId,
              contextId,
              'strategy_resolution_error',
            ),
          },
        } satisfies TaskStatusUpdateEvent)
        eventBus.finished()
        return
      }

      const resolution = resolveStrategy(candidatesResult.value, latestText)
      if (resolution.kind !== 'resolved') {
        eventBus.publish({
          kind: 'status-update',
          taskId,
          contextId,
          final: true,
          status: {
            state: 'input-required',
            timestamp: new Date().toISOString(),
            message: buildAgentMessage(
              buildClarifyingMessageText(resolution),
              taskId,
              contextId,
            ),
          },
        } satisfies TaskStatusUpdateEvent)
        eventBus.finished()
        return
      }
      strategyId = resolution.strategyId
      // The prompt handed to the strategy agent combines every user turn
      // (not just the latest), since the actual analysis instructions may
      // have been given before the disambiguating reply.
      const combinedText = userConversationText(task?.history ?? [userMessage])
      promptMessage = buildPromptMessage(combinedText, taskId, contextId)
    }

    // runStrategyAgent maps its own known failure modes to a StrategyAgentResult,
    // but an unexpected rejection (e.g. MCP client construction throwing before
    // its internal Result chain even starts) must still resolve the task
    // rather than leave it stuck in working state with eventBus.finished()
    // never called.
    // eslint-disable-next-line no-restricted-syntax -- 上記の通り、予期しない reject も捕捉して eventBus.finished() を呼び切る必要がある
    try {
      const result = await this.deps.runStrategyAgent(strategyId, promptMessage)
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
      captureWithFingerprint(error, TURN_FAILED_FINGERPRINT, {
        extras: { taskId, contextId },
      })
      eventBus.publish({
        kind: 'status-update',
        taskId,
        contextId,
        final: true,
        status: {
          state: 'failed',
          timestamp: new Date().toISOString(),
          message: buildAgentMessage(
            errorMessage(error),
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
