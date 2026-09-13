import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import { createFirstOccurrenceLabeler } from '#test/first-occurrence-labeler'

// 実行ごとに変動するタイムスタンプと executionStepId を比較用に正規化する。
export const normalizeStepTimestamps = (
  steps: readonly StrategyTaskStep[],
): unknown[] => {
  const label = createFirstOccurrenceLabeler('execution-step-id')
  return steps.map((step) => ({
    ...step,
    executionStepId: label(step.executionStepId),
    startedAt: '<started-at>',
    ...(step.finishedAt !== undefined ? { finishedAt: '<finished-at>' } : {}),
  }))
}
