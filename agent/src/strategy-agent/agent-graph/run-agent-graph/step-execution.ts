import { captureWithFingerprint } from '@fohte/service-kit/observability'
import { HumanMessage } from '@langchain/core/messages'
import type { DynamicStructuredTool } from '@langchain/core/tools'
import type { Result } from 'neverthrow'
import { err, ok } from 'neverthrow'

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
import type { AgentGraphPhase } from '#strategy-agent/agent-graph/types'
import { buildSystemPrompt } from '#strategy-agent/system-prompt'

// 上限に達しても structured response を得られなければ、そのフェーズを失敗として確定する。
const MAX_STRUCTURED_OUTPUT_ATTEMPTS = 3

const STEP_MCP_CLIENT_CLOSE_FAILED_FINGERPRINT =
  'run-agent-graph.step-mcp-client-close-failed'

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

const hasRequiredArrayFields = (
  response: Record<string, unknown>,
  requiredArrayFields: ReadonlySet<string>,
): boolean =>
  [...requiredArrayFields].every(
    (field) => Array.isArray(response[field]) && response[field].length > 0,
  )

const createPhaseAgent = (
  deps: RunAgentGraphDeps,
  phase: AgentGraphPhase,
  context: RunAgentGraphContext,
  tools: readonly DynamicStructuredTool[],
): CompiledPhaseAgent => {
  // 省略時は全 tool を許可する (単一フェーズの現行挙動と同じ)。
  const { tools: phaseTools } = phase
  const filteredTools =
    phaseTools === undefined
      ? tools
      : tools.filter((tool) => phaseTools.includes(tool.name))
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
    systemPrompt: buildSystemPrompt({
      agentsMd: context.agentsMd,
      skills: filteredSkills,
    }),
    responseSchema: buildOutputJsonSchema(phase.output),
  })
}

// invoke() 自体の reject (usage limit・ツール失敗・ネットワークエラー等) は
// 再試行せず即座に伝播する。再試行するのは invoke が成功したにもかかわらず
// structured response を欠く場合と、structured response はあるが
// requiredArrayFields (後続フェーズの for_each が要求する非空配列) を
// 満たさない場合のみで、これが再試行で解消しうる唯一の失敗モードのため。
const invokePhaseWithRetry = async (
  agent: CompiledPhaseAgent,
  messages: readonly HumanMessage[],
  executionStepId: string,
  requiredArrayFields: ReadonlySet<string>,
  attemptsLeft: number = MAX_STRUCTURED_OUTPUT_ATTEMPTS,
): Promise<Result<Record<string, unknown>, unknown>> => {
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
  if (
    structuredResponse !== undefined &&
    hasRequiredArrayFields(structuredResponse, requiredArrayFields)
  ) {
    return ok(structuredResponse)
  }
  if (attemptsLeft <= 1) {
    return err(
      structuredResponse === undefined
        ? new Error('agent did not return a structured response')
        : new Error(
            `agent's structured response did not resolve required for_each field(s) to a non-empty array: ${[...requiredArrayFields].join(', ')}`,
          ),
    )
  }
  return invokePhaseWithRetry(
    agent,
    messages,
    executionStepId,
    requiredArrayFields,
    attemptsLeft - 1,
  )
}

// 1 件分の invoke を実行し、開始時に running step を記録、決着したら
// completed/failed に更新する。for_each の各要素と、for_each でないフェーズ
// (常に 1 件) の両方から呼ばれる。
//
// spanId (OTel 未設定時は NoopTracer により固定値になる) とは独立に
// executionStepId を生成し、invokePhaseWithRetry の再試行間で使い回す。
export const invokeAndRecordStep = (
  deps: RunAgentGraphDeps,
  phase: AgentGraphPhase,
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
  // MCP の x-execution-id ヘッダーが安定し、notes.rs 側でノートが重複しない。
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
    // createStepMcpClient() 自体の同期 throw も含め、ステップの失敗として
    // 記録できるよう Promise チェーンの中で構築する。
    return (
      Promise.resolve()
        .then(() => context.createStepMcpClient(executionStepId))
        .then((stepMcpClient) =>
          Promise.resolve()
            .then(async () =>
              invokePhaseWithRetry(
                createPhaseAgent(
                  deps,
                  phase,
                  context,
                  await stepMcpClient.getTools(),
                ),
                messages,
                executionStepId,
                requiredArrayFields,
              ),
            )
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
