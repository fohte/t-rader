import { HumanMessage } from '@langchain/core/messages'
import type { Result } from 'neverthrow'
import { err, ok } from 'neverthrow'

import { findPreviousStepForPhase } from '#strategy-agent/agent-graph/resume'
import {
  getExecutionAbortMessage,
  isExecutionAborted,
} from '#strategy-agent/agent-graph/run-agent-graph/execution-abort'
import {
  collectRequiredArrayFields,
  runForEachPhase,
  splitForEach,
} from '#strategy-agent/agent-graph/run-agent-graph/for-each-runner'
import { runOperationPhase } from '#strategy-agent/agent-graph/run-agent-graph/operation-runner'
import { resolveLlmPhaseInputs } from '#strategy-agent/agent-graph/run-agent-graph/phase-inputs'
import { buildPhaseMessageText } from '#strategy-agent/agent-graph/run-agent-graph/phase-message'
import {
  createStepRecorder,
  errorMessage,
  invokeAndRecordStep,
} from '#strategy-agent/agent-graph/run-agent-graph/step-execution'
import type {
  RunAgentGraphContext,
  RunAgentGraphDeps,
  StepRecorder,
} from '#strategy-agent/agent-graph/run-agent-graph/types'
import type {
  AgentGraphConfig,
  AgentGraphPhase,
} from '#strategy-agent/agent-graph/types'
import {
  isPhaseTimeoutSignal,
  type PhaseTimeout,
  phaseTimeoutMessage,
} from '#strategy-agent/phase-timeout'
import type { StrategyAgentResult } from '#strategy-agent/strategy-agent'
import { isUsageLimitError } from '#strategy-agent/usage-limit'

export type {
  BuildPhaseAgentOptions,
  CompiledPhaseAgent,
  RunAgentGraphContext,
  RunAgentGraphDeps,
} from '#strategy-agent/agent-graph/run-agent-graph/types'

const createPhaseExecutionContext = (
  context: RunAgentGraphContext,
  phase: AgentGraphPhase,
): {
  readonly context: RunAgentGraphContext
  readonly cleanup?: () => void
} => {
  if (phase.timeoutMinutes === undefined) {
    return { context }
  }

  const phaseTimeoutController = new AbortController()
  const timeoutId = setTimeout(() => {
    phaseTimeoutController.abort()
  }, phase.timeoutMinutes * 60_000)
  const deadlineSignal =
    context.deadlineSignal === undefined
      ? phaseTimeoutController.signal
      : AbortSignal.any([context.deadlineSignal, phaseTimeoutController.signal])
  const phaseTimeout: PhaseTimeout = {
    signal: phaseTimeoutController.signal,
    phaseKey: phase.key,
    timeoutMinutes: phase.timeoutMinutes,
  }

  return {
    context: { ...context, deadlineSignal, phaseTimeout },
    cleanup: () => {
      clearTimeout(timeoutId)
    },
  }
}

const buildPhaseTimeoutResult = (
  phaseTimeout: PhaseTimeout,
): StrategyAgentResult => ({
  status: 'failed',
  message: phaseTimeoutMessage(phaseTimeout),
  errorKind: 'agent_error',
})

const buildFailureResult = (
  phase: AgentGraphPhase,
  error: unknown,
): StrategyAgentResult => ({
  status: 'failed',
  message: `フェーズ「${phase.label}」(${phase.key}) の実行に失敗しました: ${errorMessage(error)}`,
  errorKind: isUsageLimitError(error) ? 'usage_limit' : 'agent_error',
})

const runPhase = async (
  deps: RunAgentGraphDeps,
  phase: AgentGraphPhase,
  context: RunAgentGraphContext,
  priorResults: Readonly<Record<string, unknown>>,
  phaseKeys: readonly string[],
  recorder: StepRecorder,
  requiredArrayFieldsByPhase: ReadonlyMap<string, ReadonlySet<string>>,
  reuseCompleted: boolean,
): Promise<Result<unknown, unknown>> => {
  if (isExecutionAborted(context)) {
    return err(new Error(getExecutionAbortMessage(context)))
  }

  const requiredArrayFields =
    requiredArrayFieldsByPhase.get(phase.key) ?? new Set<string>()

  if (phase.kind === 'operation') {
    const previous = findPreviousStepForPhase(context.previousSteps, phase.key)
    if (
      reuseCompleted &&
      previous?.status === 'completed' &&
      previous.output !== undefined
    ) {
      recorder.recordExisting(previous)
      return ok(previous.output)
    }
    return runOperationPhase(
      phase,
      context,
      priorResults,
      recorder,
      previous?.executionStepId,
      previous?.inputRef,
    )
  }

  if (phase.forEach === undefined) {
    const previous = findPreviousStepForPhase(context.previousSteps, phase.key)
    if (reuseCompleted && previous?.status === 'completed') {
      recorder.recordExisting(previous)
      return ok(previous.output)
    }

    const resolvedInputs = await resolveLlmPhaseInputs(
      phase.input,
      priorResults,
      context,
    )
    if (resolvedInputs.isErr()) return err(resolvedInputs.error)
    if (isExecutionAborted(context)) {
      return err(new Error(getExecutionAbortMessage(context)))
    }

    const messageText = buildPhaseMessageText({
      originalPromptText: context.originalPromptText,
      phaseKeys,
      phaseKey: phase.key,
      phasePrompt: phase.prompt,
      item: undefined,
      priorResults,
      inputBindings: resolvedInputs.value,
    })
    return invokeAndRecordStep(
      deps,
      phase,
      context,
      [new HumanMessage(messageText)],
      requiredArrayFields,
      recorder,
      { phaseKey: phase.key, label: phase.label, model: phase.model },
      phase.label,
      { 'phase.key': phase.key, 'phase.model': phase.model },
      previous?.executionStepId,
    )
  }

  const [refKey, refField] = splitForEach(phase.forEach)
  const previousStepsForPhase =
    context.previousSteps?.filter((s) => s.phaseKey === phase.key) ?? []
  const resolvedInputs = await resolveLlmPhaseInputs(
    phase.input,
    priorResults,
    context,
  )
  if (resolvedInputs.isErr()) return err(resolvedInputs.error)
  if (isExecutionAborted(context)) {
    return err(new Error(getExecutionAbortMessage(context)))
  }

  return runForEachPhase(
    deps,
    phase,
    context,
    priorResults,
    resolvedInputs.value,
    phaseKeys,
    refKey,
    refField,
    recorder,
    requiredArrayFields,
    previousStepsForPhase,
    reuseCompleted,
  )
}

export const runAgentGraph = async (
  deps: RunAgentGraphDeps,
  config: AgentGraphConfig,
  context: RunAgentGraphContext,
): Promise<StrategyAgentResult> => {
  const results: Record<string, unknown> = {}
  const recorder = createStepRecorder(context.onStepsChanged)
  const phaseKeys = config.phases.map((phase) => phase.key)
  const requiredArrayFieldsByPhase = collectRequiredArrayFields(config.phases)

  // for_each は 1 件でも成功すればフェーズ成功として返すため、部分失敗したタスクは
  // 失敗した要素を抱えたまま後続フェーズまで完了している。やり直して復活した要素の出力を
  // 後続フェーズに反映するには、最初の未完了ステップを含むフェーズより後ろは
  // completed でも再実行する必要がある。
  const firstIncompleteIndex = config.phases.findIndex(
    (phase) =>
      context.previousSteps?.some(
        (s) => s.phaseKey === phase.key && s.status !== 'completed',
      ) === true,
  )

  for (const [index, phase] of config.phases.entries()) {
    const phaseExecution = createPhaseExecutionContext(context, phase)
    const phaseResult = await runPhase(
      deps,
      phase,
      phaseExecution.context,
      results,
      phaseKeys,
      recorder,
      requiredArrayFieldsByPhase,
      firstIncompleteIndex === -1 || index <= firstIncompleteIndex,
    ).finally(() => {
      phaseExecution.cleanup?.()
    })

    const { phaseTimeout } = phaseExecution.context
    if (
      isPhaseTimeoutSignal(phaseExecution.context.deadlineSignal, phaseTimeout)
    ) {
      return buildPhaseTimeoutResult(phaseTimeout)
    }
    if (phaseResult.isErr()) {
      if (
        phaseExecution.context.phaseTimeout !== undefined &&
        isExecutionAborted(phaseExecution.context)
      ) {
        return buildFailureResult(
          phase,
          new Error(getExecutionAbortMessage(phaseExecution.context), {
            cause: phaseResult.error,
          }),
        )
      }
      return buildFailureResult(phase, phaseResult.error)
    }
    results[phase.key] = phaseResult.value
  }

  return {
    status: 'completed',
    message: `${String(config.phases.length)}フェーズの実行が完了しました (${config.phases.map((p) => p.label).join(' → ')})`,
  }
}
