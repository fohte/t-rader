import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'

// 実行ごとに変動するタイムスタンプと executionStepId を比較用に正規化する。
export const normalizeStepTimestamps = (
  steps: readonly StrategyTaskStep[],
): unknown[] => {
  const labels = new Map<string, string>()
  return steps.map((step) => {
    let label = labels.get(step.executionStepId)
    if (label === undefined) {
      label = `<execution-step-id-${String(labels.size + 1)}>`
      labels.set(step.executionStepId, label)
    }
    return {
      ...step,
      executionStepId: label,
      startedAt: '<started-at>',
      ...(step.finishedAt !== undefined ? { finishedAt: '<finished-at>' } : {}),
    }
  })
}
