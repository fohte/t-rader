import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'

export const findPreviousStepForPhase = (
  previousSteps: readonly StrategyTaskStep[] | undefined,
  phaseKey: string,
): StrategyTaskStep | undefined =>
  previousSteps?.find((s) => s.phaseKey === phaseKey)

export interface PreviousStepMatcher {
  readonly take: (item: unknown) => StrategyTaskStep | undefined
}

// マッチ済みの previous step をここから取り除きながら消費する。取り除か
// ないと、item の内容が重複する要素が複数ある場合に同じ previous step
// (同じ executionStepId) へ複数の item がマッチしてしまう。
export const createPreviousStepMatcher = (
  previousStepsForPhase: readonly StrategyTaskStep[],
): PreviousStepMatcher => {
  const remaining = [...previousStepsForPhase]
  return {
    // item の値そのもので前回実行との対応を取る (for_each の要素には
    // 安定した ID が無いため)。各 worker は item を取得してから処理を await するため、
    // 共有配列から重複 item に対応する previous step を順に消費できる。
    take: (item) => {
      const matchedIndex = remaining.findIndex(
        (s) => JSON.stringify(s.item) === JSON.stringify(item),
      )
      return matchedIndex === -1
        ? undefined
        : remaining.splice(matchedIndex, 1)[0]
    },
  }
}
