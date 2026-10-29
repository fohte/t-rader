import { captureWithFingerprint } from '@fohte/service-kit/observability'
import { HumanMessage } from '@langchain/core/messages'
import type { DynamicStructuredTool } from '@langchain/core/tools'
import type { Result } from 'neverthrow'
import { err, ok } from 'neverthrow'

import { logger } from '#logger'
import { buildOutputJsonSchema } from '#strategy-agent/agent-graph/output-schema'
import type {
  CompiledPhaseAgent,
  RunAgentGraphContext,
  RunAgentGraphDeps,
  StepRecorder,
  StepStartInput,
} from '#strategy-agent/agent-graph/run-agent-graph/types'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import { withPhaseSpan } from '#strategy-agent/agent-graph/tracing'
import type { LlmAgentGraphPhase } from '#strategy-agent/agent-graph/types'
import type { McpToolsClient } from '#strategy-agent/strategy-agent'
import { buildSystemPrompt } from '#strategy-agent/system-prompt'

// 上限に達しても structured response を得られなければ、そのフェーズを失敗として確定する。
const MAX_STRUCTURED_OUTPUT_ATTEMPTS = 3

const STEP_MCP_CLIENT_CLOSE_FAILED_FINGERPRINT =
  'run-agent-graph.step-mcp-client-close-failed'
const STRUCTURED_OUTPUT_RETRY_FINGERPRINT =
  'run-agent-graph.structured-output-retry'

const EXECUTION_RUNTIME_TOOL_NAMES = new Set([
  'capture_dataset',
  'read_task_execution_snapshot',
])

export const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

// steps 配列はここでのみ mutate する。呼び出し側 (executor) は
// onStepsChanged で渡された配列を都度「最新の全体」として扱えばよい。
export const createStepRecorder = (
  onStepsChanged: RunAgentGraphContext['onStepsChanged'],
): StepRecorder => {
  const steps: StrategyTaskStep[] = []
  const notify = (): void => onStepsChanged?.(steps.slice())

  return {
    start: (step) => {
      steps.push({ ...step, status: 'running' })
      notify()
      return steps.length - 1
    },
    update: (index, update) => {
      const current = steps[index]
      if (current === undefined) return
      steps[index] = { ...current, ...update }
      notify()
    },
    finish: (index, outcome) => {
      const current = steps[index]
      if (current === undefined) return
      steps[index] = {
        ...current,
        ...outcome,
        finishedAt: new Date().toISOString(),
      }
      notify()
    },
    recordExisting: (step) => {
      steps.push(step)
      notify()
    },
  }
}

type StructuredOutputRejectionReason =
  | 'missing_structured_output'
  | 'missing_required_array_fields'
  | 'missing_required_tools'

interface StructuredOutputRejection {
  missingStructuredOutput: boolean
  missingRequiredArrayFields: readonly string[]
  missingRequiredTools: readonly string[]
}

const getRejectionReason = (
  rejection: StructuredOutputRejection,
): StructuredOutputRejectionReason =>
  rejection.missingStructuredOutput
    ? 'missing_structured_output'
    : rejection.missingRequiredArrayFields.length > 0
      ? 'missing_required_array_fields'
      : 'missing_required_tools'

const buildRejectionMessage = (
  rejection: StructuredOutputRejection,
): string => {
  const reasons: string[] = []
  if (rejection.missingStructuredOutput) {
    reasons.push('agent did not return a structured response')
  }
  if (rejection.missingRequiredArrayFields.length > 0) {
    reasons.push(
      `agent's structured response did not resolve required for_each field(s) to an array: ${rejection.missingRequiredArrayFields.join(', ')}`,
    )
  }
  if (rejection.missingRequiredTools.length > 0) {
    reasons.push(
      `required tool(s) did not complete successfully: ${rejection.missingRequiredTools.join(', ')}`,
    )
  }
  return reasons.join('; ')
}

const buildRetryFeedbackMessage = (
  rejection: StructuredOutputRejection,
): HumanMessage => {
  const instructions: string[] = []
  if (
    rejection.missingStructuredOutput ||
    rejection.missingRequiredArrayFields.length > 0
  ) {
    instructions.push(
      '構造化出力 tool を呼び出して、必須フィールドを満たす内容を提出し直してください。',
    )
  }
  if (rejection.missingRequiredTools.length > 0) {
    instructions.push(
      `未成功の必須 tool を実行してから、構造化出力を提出し直してください: ${rejection.missingRequiredTools.join(', ')}`,
    )
  }
  return new HumanMessage(
    [
      `前回の試行は却下されました (理由: ${buildRejectionMessage(rejection)})。`,
      ...instructions,
      'write_note・create_annotation などの書き込み系 tool は前回の試行で' +
        '既に実行済みの可能性があります。同じ内容を重複して実行しないでください。',
    ].join('\n'),
  )
}

const createPhaseAgent = (
  deps: RunAgentGraphDeps,
  phase: LlmAgentGraphPhase,
  context: RunAgentGraphContext,
  tools: readonly DynamicStructuredTool[],
  localTools: readonly DynamicStructuredTool[],
  mcpClient: McpToolsClient,
  onModelCallLimitReached: () => void,
  onToolSuccess: ((toolName: string) => void) | undefined,
): CompiledPhaseAgent => {
  // 省略時は全 tool を許可する (単一フェーズの現行挙動と同じ)。
  const { tools: phaseTools } = phase
  const filteredMcpTools = tools.filter(
    (tool) =>
      !EXECUTION_RUNTIME_TOOL_NAMES.has(tool.name) &&
      (phaseTools === undefined || phaseTools.includes(tool.name)),
  )
  const filteredTools = [...filteredMcpTools, ...localTools]
  const filteredSkills: Record<string, string> = {}
  for (const name of phase.skills) {
    const body = context.skills[name]
    if (body !== undefined) filteredSkills[name] = body
  }
  return deps.buildPhaseAgent({
    model: deps.createChatModel(phase.model, {
      ...(phase.reasoningEffort !== undefined
        ? { reasoningEffort: phase.reasoningEffort }
        : {}),
    }),
    tools: filteredTools,
    mcpTools: filteredMcpTools,
    reserveModelCall: (callId) => mcpClient.reserveModelCall(callId),
    transitionReservation: (callId, state) =>
      mcpClient.transitionReservation(callId, state),
    systemPrompt: buildSystemPrompt({
      agentsMd: context.agentsMd,
      skills: filteredSkills,
    }),
    responseSchema: buildOutputJsonSchema(phase.output),
    ...(context.deadlineSignal !== undefined
      ? { deadlineSignal: context.deadlineSignal }
      : {}),
    ...(context.phaseTimeout !== undefined
      ? { phaseTimeout: context.phaseTimeout }
      : {}),
    onModelCallLimitReached,
    ...(onToolSuccess !== undefined ? { onToolSuccess } : {}),
  })
}

const resolveLocalTools = (
  phase: LlmAgentGraphPhase,
  executionStepId: string,
  recorder: StepRecorder,
  context: RunAgentGraphContext,
  parentMcpClient: McpToolsClient,
): Result<DynamicStructuredTool[], Error> => {
  if (phase.subagents.length === 0) return ok([])
  const factory = context.createSubagentTool
  if (factory === undefined) {
    return err(
      new Error(
        `phase ${phase.key} declares subagents but no subagent tool factory is configured`,
      ),
    )
  }
  return ok([
    factory(phase, executionStepId, recorder, context, parentMcpClient),
  ])
}

// invoke() 自体の reject (usage limit・ツール失敗・ネットワークエラー等) は
// 再試行せず即座に伝播する。再試行するのは invoke が成功したにもかかわらず
// structured response を欠く場合、requiredArrayFields (後続フェーズの for_each が
// 要求する配列) を満たさない場合、または requiredTools が成功していない場合。
//
// invoke() は attempt ごとに新規の LangGraph 実行になり、前の attempt の
// AIMessage/ToolMessage を引き継がない。却下理由は次の attempt の messages に
// 積み増すことでのみモデルへ伝わる。
interface InvokePhaseWithRetryOptions {
  agent: CompiledPhaseAgent
  messages: readonly HumanMessage[]
  executionStepId: string
  requiredArrayFields: ReadonlySet<string>
  getMissingRequiredTools: () => readonly string[]
  phaseKey: string
  attemptsLeft?: number
}

const invokePhaseWithRetry = async ({
  agent,
  messages,
  executionStepId,
  requiredArrayFields,
  getMissingRequiredTools,
  phaseKey,
  attemptsLeft = MAX_STRUCTURED_OUTPUT_ATTEMPTS,
}: InvokePhaseWithRetryOptions): Promise<
  Result<Record<string, unknown>, unknown>
> => {
  const invoked = await agent.invoke({ messages, executionStepId }).then(
    (
      value,
    ): Result<{ structuredResponse?: Record<string, unknown> }, unknown> =>
      ok(value),
    (
      error: unknown,
    ): Result<{ structuredResponse?: Record<string, unknown> }, unknown> =>
      err(error),
  )
  if (invoked.isErr()) return invoked
  const { structuredResponse } = invoked.value
  const rejection: StructuredOutputRejection = {
    missingStructuredOutput: structuredResponse === undefined,
    missingRequiredArrayFields:
      structuredResponse === undefined
        ? []
        : [...requiredArrayFields].filter(
            (field) => !Array.isArray(structuredResponse[field]),
          ),
    missingRequiredTools: getMissingRequiredTools(),
  }
  if (
    structuredResponse !== undefined &&
    rejection.missingRequiredArrayFields.length === 0 &&
    rejection.missingRequiredTools.length === 0
  ) {
    return ok(structuredResponse)
  }
  const reason = getRejectionReason(rejection)
  if (attemptsLeft <= 1) {
    return err(new Error(buildRejectionMessage(rejection)))
  }

  const attemptNumber = MAX_STRUCTURED_OUTPUT_ATTEMPTS - attemptsLeft + 1
  const retryError = new Error(
    `phase "${phaseKey}" (executionStepId=${executionStepId}) structured output attempt ${String(attemptNumber)} rejected (${reason}), retrying`,
  )
  logger.warn({ executionStepId, attemptNumber, reason }, retryError.message)
  captureWithFingerprint(retryError, STRUCTURED_OUTPUT_RETRY_FINGERPRINT, {
    level: 'warning',
    extras: { phaseKey, executionStepId, attemptNumber, reason },
  })

  return invokePhaseWithRetry({
    agent,
    messages: [...messages, buildRetryFeedbackMessage(rejection)],
    executionStepId,
    requiredArrayFields,
    getMissingRequiredTools,
    phaseKey,
    attemptsLeft: attemptsLeft - 1,
  })
}

// 1 件分の invoke を実行し、開始時に running step を記録、決着したら
// completed/failed に更新する。for_each の各要素と、for_each でないフェーズ
// (常に 1 件) の両方から呼ばれる。
//
// spanId (OTel 未設定時は NoopTracer により固定値になる) とは独立に
// executionStepId を生成し、invokePhaseWithRetry の再試行間で使い回す。
export const invokeAndRecordStep = (
  deps: RunAgentGraphDeps,
  phase: LlmAgentGraphPhase,
  context: RunAgentGraphContext,
  messages: readonly HumanMessage[],
  requiredArrayFields: ReadonlySet<string>,
  recorder: StepRecorder,
  stepBase: Omit<
    StepStartInput,
    'startedAt' | 'traceId' | 'spanId' | 'executionStepId'
  >,
  spanName: string,
  spanAttributes: Record<string, string | number>,
  // resume で再実行するステップの元 executionStepId。指定時はこれを使い回すことで
  // MCP の x-execution-id ヘッダーに含まれる step_id 部分が安定し、
  // notes.rs 側 (execution_id で紐付け) でノートが重複しない。
  existingExecutionStepId?: string,
): Promise<Result<Record<string, unknown>, unknown>> =>
  withPhaseSpan(spanName, spanAttributes, (spanIds) => {
    const executionStepId = existingExecutionStepId ?? crypto.randomUUID()
    const index = recorder.start({
      ...stepBase,
      executionStepId,
      startedAt: new Date().toISOString(),
      traceId: spanIds.traceId,
      spanId: spanIds.spanId,
    })
    let modelCallLimitReachedRecorded = false
    const requiredTools = new Set(phase.requiredTools ?? [])
    const successfulTools = new Set<string>()
    const getMissingRequiredTools = () =>
      [...requiredTools].filter((tool) => !successfulTools.has(tool))
    // createStepMcpClient() 自体の同期 throw も含め、ステップの失敗として
    // 記録できるよう Promise チェーンの中で構築する。
    return (
      Promise.resolve()
        .then(() => context.createStepMcpClient(executionStepId))
        .then((stepMcpClient) =>
          Promise.resolve()
            .then(async () => {
              const mcpTools = await stepMcpClient.getTools()
              const localTools = resolveLocalTools(
                phase,
                executionStepId,
                recorder,
                context,
                stepMcpClient,
              )
              if (localTools.isErr()) return err(localTools.error)
              return invokePhaseWithRetry({
                agent: createPhaseAgent(
                  deps,
                  phase,
                  context,
                  mcpTools,
                  localTools.value,
                  stepMcpClient,
                  // 再提出が失敗しても、この step が上限に達した事実を残す。
                  () => {
                    if (modelCallLimitReachedRecorded) return
                    modelCallLimitReachedRecorded = true
                    recorder.update(index, { modelCallLimitReached: true })
                  },
                  requiredTools.size > 0
                    ? (toolName) => successfulTools.add(toolName)
                    : undefined,
                ),
                messages,
                executionStepId,
                requiredArrayFields,
                getMissingRequiredTools,
                phaseKey: phase.key,
              })
            })
            .finally(() =>
              // close の失敗でステップの決着を取り消さない。
              stepMcpClient.close().catch((closeError: unknown) => {
                captureWithFingerprint(
                  closeError,
                  STEP_MCP_CLIENT_CLOSE_FAILED_FINGERPRINT,
                  { extras: { executionStepId } },
                )
              }),
            ),
        )
        // createStepMcpClient()/getTools() の失敗 (コネクションを張れない等) も
        // invoke の失敗と同じ経路でステップの失敗として記録する。
        .catch((error: unknown) => err(error))
        .then((result) => {
          recorder.finish(
            index,
            result.isErr()
              ? { status: 'failed', error: errorMessage(result.error) }
              : { status: 'completed', output: result.value },
          )
          return result
        })
    )
  })
