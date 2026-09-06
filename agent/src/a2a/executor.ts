import { randomUUID } from 'node:crypto'

import type {
  Message,
  Task,
  TaskArtifactUpdateEvent,
  TaskStatusUpdateEvent,
} from '@a2a-js/sdk'
import type {
  AgentExecutor,
  ExecutionEventBus,
  RequestContext,
  TaskStore,
} from '@a2a-js/sdk/server'
import { captureWithFingerprint } from '@fohte/service-kit/observability'

import { extractMessageText } from '#a2a/message-text'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import {
  AGENT_GRAPH_STEPS_ARTIFACT_ID,
  toStepJson,
} from '#strategy-agent/agent-graph/step'
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

// watchdog のデフォルトタイムアウト (env.ts の A2A_WATCHDOG_TIMEOUT_MS、10分)
// より十分短く保つ。単一フェーズが for_each を含まず、開始から終了まで
// step の変化が一切ないまま長時間かかっても heartbeat を止めないための間隔。
export const HEARTBEAT_INTERVAL_MS = 3 * 60 * 1000

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
  messageId: string = randomUUID(),
): Message => ({
  kind: 'message',
  role: 'agent',
  messageId,
  taskId,
  contextId,
  parts: [{ kind: 'text', text }],
  ...(errorKind !== undefined ? { metadata: { error_kind: errorKind } } : {}),
})

const STEP_STATUS_LABELS: Record<StrategyTaskStep['status'], string> = {
  running: '実行中',
  completed: '完了',
  failed: '失敗',
}

const changedAt = (step: StrategyTaskStep): string =>
  step.finishedAt ?? step.startedAt

// for_each は maxParallel 個の要素を並列実行するため、配列の末尾が直近に
// start/finish した要素とは限らない (先に start した要素が後に finish
// することがある)。changedAt が最大の要素を実際の「直近の変化」として選ぶ。
const mostRecentlyChanged = (
  steps: readonly StrategyTaskStep[],
): StrategyTaskStep | undefined =>
  steps.reduce<StrategyTaskStep | undefined>(
    (latest, step) =>
      latest === undefined || changedAt(step) > changedAt(latest)
        ? step
        : latest,
    undefined,
  )

const buildStepsProgressMessageText = (
  steps: readonly StrategyTaskStep[],
): string => {
  const current = mostRecentlyChanged(steps)
  if (current === undefined) return '実行中'
  const samePhase = steps.filter((step) => step.phaseKey === current.phaseKey)
  const position = samePhase.indexOf(current) + 1
  const progress =
    samePhase.length > 1
      ? `(${String(position)}/${String(samePhase.length)})`
      : ''
  const item = current.itemLabel !== undefined ? `: ${current.itemLabel}` : ''
  const suffix = `${progress}${item}`
  return `フェーズ「${current.label}」${suffix}${suffix === '' ? '' : ' '}が${STEP_STATUS_LABELS[current.status]}`
}

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
    taskId: string,
    userMessage: Message,
    onStepsChanged?: (steps: readonly StrategyTaskStep[]) => void,
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

    // agent_graph の各フェーズ/for_each 要素の進捗を artifact-update として
    // publish する。同じ artifactId を append: false (省略時のデフォルト) で
    // 都度置換することで、Task.history を汚さずに Task.artifacts 経由で
    // 永続化される。
    //
    // messageId を固定することで Task.status.message のみを更新し
    // Task.history への蓄積を防ぐ。ResultManager の messageId 重複排除
    // という内部実装依存の挙動であり、@a2a-js/sdk の公開契約ではない。
    const stepsHeartbeatMessageId = randomUUID()
    let latestSteps: readonly StrategyTaskStep[] = []
    // watchdog の heartbeat は working status-update の timestamp でのみ
    // 進む (lifecycle.ts 参照)。artifact-update はそれを進めない。
    const publishHeartbeat = (steps: readonly StrategyTaskStep[]): void => {
      eventBus.publish({
        kind: 'status-update',
        taskId,
        contextId,
        final: false,
        status: {
          state: 'working',
          timestamp: new Date().toISOString(),
          message: buildAgentMessage(
            buildStepsProgressMessageText(steps),
            taskId,
            contextId,
            undefined,
            stepsHeartbeatMessageId,
          ),
        },
      } satisfies TaskStatusUpdateEvent)
    }
    const publishSteps = (steps: readonly StrategyTaskStep[]): void => {
      latestSteps = steps
      eventBus.publish({
        kind: 'artifact-update',
        taskId,
        contextId,
        artifact: {
          artifactId: AGENT_GRAPH_STEPS_ARTIFACT_ID,
          name: AGENT_GRAPH_STEPS_ARTIFACT_ID,
          parts: [{ kind: 'data', data: { steps: steps.map(toStepJson) } }],
        },
      } satisfies TaskArtifactUpdateEvent)
      publishHeartbeat(steps)
    }

    // runStrategyAgent maps its own known failure modes to a StrategyAgentResult,
    // but an unexpected rejection (e.g. MCP client construction throwing before
    // its internal Result chain even starts) must still resolve the task
    // rather than leave it stuck in working state with eventBus.finished()
    // never called.
    // publishSteps は step 変化時にしか heartbeat を出さないため、変化が
    // ない間も HEARTBEAT_INTERVAL_MS ごとに再送する。
    const heartbeatTimer = setInterval(() => {
      publishHeartbeat(latestSteps)
    }, HEARTBEAT_INTERVAL_MS)

    // eslint-disable-next-line no-restricted-syntax -- 上記の通り、予期しない reject も捕捉して eventBus.finished() を呼び切る必要がある
    try {
      const result = await this.deps.runStrategyAgent(
        strategyId,
        taskId,
        promptMessage,
        publishSteps,
      )
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
      clearInterval(heartbeatTimer)
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
