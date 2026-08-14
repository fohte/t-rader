// agent_graph の 1 フェーズ (for_each の場合は要素 1 件) の実行状況。backend の
// strategy_task.steps (jsonb, フラット配列) にそのまま乗る契約 (backend は中身を
// 解釈せず素通しする) なので、キー名は他の内部 API レスポンスと同じ snake_case にする。
export const AGENT_GRAPH_STEPS_ARTIFACT_ID = 'agent-graph-steps'

export type StrategyTaskStepStatus = 'running' | 'completed' | 'failed'

export interface StrategyTaskStep {
  readonly phaseKey: string
  readonly label: string
  readonly model: string
  readonly status: StrategyTaskStepStatus
  readonly item?: unknown
  readonly itemLabel?: string
  readonly output?: unknown
  readonly startedAt: string
  readonly finishedAt?: string
  readonly traceId: string
  readonly spanId: string
  readonly error?: string
}

export const toStepJson = (
  step: StrategyTaskStep,
): Record<string, unknown> => ({
  phase_key: step.phaseKey,
  label: step.label,
  model: step.model,
  status: step.status,
  ...(step.item !== undefined ? { item: step.item } : {}),
  ...(step.itemLabel !== undefined ? { item_label: step.itemLabel } : {}),
  ...(step.output !== undefined ? { output: step.output } : {}),
  started_at: step.startedAt,
  ...(step.finishedAt !== undefined ? { finished_at: step.finishedAt } : {}),
  trace_id: step.traceId,
  span_id: step.spanId,
  ...(step.error !== undefined ? { error: step.error } : {}),
})
