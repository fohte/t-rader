import { HumanMessage } from '@langchain/core/messages'
import type { Result } from 'neverthrow'
import { err, ok } from 'neverthrow'

import { isPlainObject } from '#strategy-agent/agent-graph/json'
import { createPreviousStepMatcher } from '#strategy-agent/agent-graph/resume'
import {
  getExecutionAbortMessage,
  isExecutionAborted,
} from '#strategy-agent/agent-graph/run-agent-graph/execution-abort'
import {
  payloadPointerForField,
  readSealedPayloadPages,
} from '#strategy-agent/agent-graph/run-agent-graph/payload-for-each'
import { buildPhaseMessageText } from '#strategy-agent/agent-graph/run-agent-graph/phase-message'
import {
  errorMessage,
  invokeAndRecordStep,
} from '#strategy-agent/agent-graph/run-agent-graph/step-execution'
import type {
  RunAgentGraphContext,
  RunAgentGraphDeps,
  StepRecorder,
} from '#strategy-agent/agent-graph/run-agent-graph/types'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import type {
  AgentGraphPhase,
  LlmAgentGraphPhase,
} from '#strategy-agent/agent-graph/types'
import { isUsageLimitError } from '#strategy-agent/usage-limit'

interface ForEachBatch {
  readonly outputs: readonly unknown[]
  readonly firstError?: unknown
  readonly itemCount: number
}

type ForEachItemOutcome =
  | { readonly kind: 'success'; readonly value: unknown }
  | { readonly kind: 'error'; readonly error: unknown }

interface ForEachExecutionOptions {
  readonly deps: RunAgentGraphDeps
  readonly phase: LlmAgentGraphPhase
  readonly context: RunAgentGraphContext
  readonly priorResults: Readonly<Record<string, unknown>>
  readonly inputBindings: Readonly<Record<string, unknown>>
  readonly phaseKeys: readonly string[]
  readonly recorder: StepRecorder
  readonly requiredArrayFields: ReadonlySet<string>
  readonly previousStepsForPhase: readonly StrategyTaskStep[]
  readonly reuseCompleted: boolean
}

interface RunForEachItemsOptions extends ForEachExecutionOptions {
  readonly items: readonly unknown[]
  readonly itemKeys?: readonly unknown[]
  readonly startIndex?: number
  readonly totalItemCount?: number
}

interface RunPayloadBackedForEachOptions extends ForEachExecutionOptions {
  readonly payloadRef: { readonly id: string; readonly digest?: string }
  readonly pointer: string
}

interface RunForEachPhaseOptions extends ForEachExecutionOptions {
  readonly refKey: string
  readonly refField: string
}

const PAYLOAD_FOR_EACH_PAGE_SIZE = 100

export const splitForEach = (forEach: string): readonly [string, string] => {
  const dotIndex = forEach.indexOf('.')
  return [forEach.slice(0, dotIndex), forEach.slice(dotIndex + 1)]
}

export const collectRequiredArrayFields = (
  phases: readonly AgentGraphPhase[],
): ReadonlyMap<string, ReadonlySet<string>> => {
  const map = new Map<string, Set<string>>()
  for (const phase of phases) {
    if (phase.forEach === undefined) continue
    const [refKey, refField] = splitForEach(phase.forEach)
    const fields = map.get(refKey) ?? new Set<string>()
    fields.add(refField)
    map.set(refKey, fields)
  }
  return map
}

const resolveForEachItems = (
  priorResults: Readonly<Record<string, unknown>>,
  refKey: string,
  refField: string,
): Result<readonly unknown[], Error> => {
  const referenced = priorResults[refKey]
  const items = isPlainObject(referenced) ? referenced[refField] : undefined
  if (!Array.isArray(items)) {
    return err(
      new Error(
        `for_each の参照先 "${refKey}.${refField}" が配列ではありません`,
      ),
    )
  }
  return ok(items)
}

const buildForEachFailure = (itemCount: number, error: unknown): unknown =>
  isUsageLimitError(error)
    ? error
    : new Error(
        `for_each の全要素 (${String(itemCount)}件) が失敗しました: ${errorMessage(error)}`,
      )

const extractItemLabel = (
  item: unknown,
  labelField: string | undefined,
): string | undefined => {
  if (labelField === undefined || !isPlainObject(item)) return undefined
  const value = item[labelField]
  return typeof value === 'string' ? value : undefined
}

const runForEachItems = async ({
  deps,
  phase,
  context,
  priorResults,
  inputBindings,
  phaseKeys,
  items,
  recorder,
  requiredArrayFields,
  previousStepsForPhase,
  reuseCompleted,
  itemKeys,
  startIndex = 0,
  totalItemCount = items.length,
}: RunForEachItemsOptions): Promise<Result<ForEachBatch, unknown>> => {
  const concurrency = Math.max(phase.maxParallel ?? items.length, 1)
  const workerCount = Math.min(concurrency, items.length)
  const results: (ForEachItemOutcome | undefined)[] = []
  let nextIndex = 0
  const outputs: unknown[] = []
  let firstError: unknown
  const previousStepMatcher = createPreviousStepMatcher(previousStepsForPhase)

  if (isExecutionAborted(context)) {
    return err(new Error(getExecutionAbortMessage(context)))
  }

  const runItem = (index: number): Promise<Result<unknown, unknown>> => {
    const item = items[index]
    const itemLabel = extractItemLabel(item, phase.labelField)
    const itemKey = itemKeys?.[index]
    const matched = previousStepMatcher.take(item, itemKey)
    if (reuseCompleted && matched?.status === 'completed') {
      recorder.recordExisting(matched)
      return Promise.resolve(ok(matched.output))
    }
    const messageText = buildPhaseMessageText({
      originalPromptText: context.originalPromptText,
      phaseKeys,
      phaseKey: phase.key,
      phasePrompt: phase.prompt,
      item,
      priorResults,
      inputBindings,
    })
    return invokeAndRecordStep(
      deps,
      phase,
      context,
      [new HumanMessage(messageText)],
      requiredArrayFields,
      recorder,
      {
        phaseKey: phase.key,
        label: phase.label,
        model: phase.model,
        item,
        ...(itemKey !== undefined ? { itemKey } : {}),
        ...(itemLabel !== undefined ? { itemLabel } : {}),
      },
      `${phase.label} (${String(startIndex + index + 1)}/${String(totalItemCount)})`,
      {
        'phase.key': phase.key,
        'phase.model': phase.model,
        'phase.item_index': startIndex + index,
      },
      matched?.executionStepId,
    )
  }

  const runWorker = async (): Promise<boolean> => {
    let isFirstItem = true
    while (nextIndex < items.length) {
      if (!isFirstItem && isExecutionAborted(context)) return true
      const index = nextIndex
      nextIndex++
      isFirstItem = false
      const itemResult = await runItem(index)
      results[index] = itemResult.match(
        (value): ForEachItemOutcome => ({ kind: 'success', value }),
        (error): ForEachItemOutcome => ({ kind: 'error', error }),
      )
    }
    return false
  }

  const workerDeadlines = await Promise.all(
    Array.from({ length: workerCount }, () => runWorker()),
  )

  if (workerDeadlines.includes(true)) {
    return err(new Error(getExecutionAbortMessage(context)))
  }

  for (const itemResult of results) {
    if (itemResult === undefined) continue
    if (itemResult.kind === 'error') firstError ??= itemResult.error
    else outputs.push(itemResult.value)
  }

  return ok({
    outputs,
    ...(firstError !== undefined ? { firstError } : {}),
    itemCount: items.length,
  })
}

const runPayloadBackedForEach = async ({
  deps,
  phase,
  context,
  priorResults,
  inputBindings,
  phaseKeys,
  payloadRef,
  pointer,
  recorder,
  requiredArrayFields,
  previousStepsForPhase,
  reuseCompleted,
}: RunPayloadBackedForEachOptions): Promise<Result<unknown[], unknown>> => {
  if (isExecutionAborted(context)) {
    return err(new Error(getExecutionAbortMessage(context)))
  }
  const outputs: unknown[] = []
  let firstError: unknown
  const readResult = await Promise.resolve()
    .then(() => context.createStepMcpClient())
    .then((client) =>
      readSealedPayloadPages(
        client,
        payloadRef,
        pointer,
        PAYLOAD_FOR_EACH_PAGE_SIZE,
        async (pageItems, totalItemCount) => {
          const batch = await runForEachItems({
            deps,
            phase,
            context,
            priorResults,
            inputBindings,
            phaseKeys,
            items: pageItems.map((item) => item.value),
            recorder,
            requiredArrayFields,
            previousStepsForPhase,
            reuseCompleted,
            itemKeys: pageItems.map((item) => item.itemKey),
            startIndex: pageItems[0]?.itemKey.index ?? 0,
            totalItemCount,
          })
          if (batch.isErr()) return err(batch.error)
          outputs.push(...batch.value.outputs)
          firstError ??= batch.value.firstError
          return ok(undefined)
        },
        context.deadlineSignal,
      ).finally(() => client.close().catch(() => undefined)),
    )
    .catch((error: unknown) =>
      err(error instanceof Error ? error : new Error(String(error))),
    )

  if (readResult.isErr()) return err(readResult.error)
  if (readResult.value > 0 && outputs.length === 0) {
    return err(buildForEachFailure(readResult.value, firstError))
  }
  return ok(outputs)
}

export const runForEachPhase = async ({
  deps,
  phase,
  context,
  priorResults,
  inputBindings,
  phaseKeys,
  refKey,
  refField,
  recorder,
  requiredArrayFields,
  previousStepsForPhase,
  reuseCompleted,
}: RunForEachPhaseOptions): Promise<Result<unknown, unknown>> => {
  const payloadSource = payloadPointerForField(priorResults[refKey], refField)
  if (payloadSource !== undefined) {
    return runPayloadBackedForEach({
      deps,
      phase,
      context,
      priorResults,
      inputBindings,
      phaseKeys,
      payloadRef: payloadSource.payloadRef,
      pointer: payloadSource.pointer,
      recorder,
      requiredArrayFields,
      previousStepsForPhase,
      reuseCompleted,
    })
  }

  const itemsResult = resolveForEachItems(priorResults, refKey, refField)
  if (itemsResult.isErr()) return itemsResult

  const batch = await runForEachItems({
    deps,
    phase,
    context,
    priorResults,
    inputBindings,
    phaseKeys,
    items: itemsResult.value,
    recorder,
    requiredArrayFields,
    previousStepsForPhase,
    reuseCompleted,
  })
  if (batch.isErr()) return batch
  if (batch.value.itemCount > 0 && batch.value.outputs.length === 0) {
    return err(
      buildForEachFailure(batch.value.itemCount, batch.value.firstError),
    )
  }
  return ok([...batch.value.outputs])
}
