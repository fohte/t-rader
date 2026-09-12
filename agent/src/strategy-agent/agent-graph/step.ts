import { z } from 'zod'

// agent_graph の 1 フェーズ (for_each の場合は要素 1 件) の実行状況。backend の
// strategy_task.steps (jsonb, フラット配列) にそのまま乗る契約 (backend は中身を
// 解釈せず素通しする) なので、キー名は他の内部 API レスポンスと同じ snake_case にする。
export const AGENT_GRAPH_STEPS_ARTIFACT_ID = 'agent-graph-steps'

export type StrategyTaskStepStatus = 'running' | 'completed' | 'failed'

export interface StrategyTaskStep {
  readonly phaseKey: string
  readonly executionStepId: string
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

// steps に乗る wire 形式 (snake_case)。frontend はこの型を直接参照して narrow する。
export interface StrategyTaskStepJson {
  readonly phase_key: string
  readonly execution_step_id: string
  readonly label: string
  readonly model: string
  readonly status: StrategyTaskStepStatus
  readonly item?: unknown
  readonly item_label?: string
  readonly output?: unknown
  readonly started_at: string
  readonly finished_at?: string
  readonly trace_id: string
  readonly span_id: string
  readonly error?: string
}

export const toStepJson = (step: StrategyTaskStep): StrategyTaskStepJson => ({
  phase_key: step.phaseKey,
  execution_step_id: step.executionStepId,
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

// resume 用に backend (`POST /internal/tasks` の `resume_steps`) から届く wire JSON
// の形状。中身は backend 側 (ResumeStepWireJson) の契約で保証されている前提だが、
// 型システムの外から届く値のため実行時にも検証する。
export const strategyTaskStepJsonSchema = z.object({
  phase_key: z.string(),
  execution_step_id: z.string(),
  label: z.string(),
  model: z.string(),
  status: z.enum(['running', 'completed', 'failed']),
  item: z.unknown().optional(),
  item_label: z.string().optional(),
  output: z.unknown().optional(),
  started_at: z.string(),
  finished_at: z.string().optional(),
  trace_id: z.string(),
  span_id: z.string(),
  error: z.string().optional(),
})

export const fromStepJson = (
  json: z.infer<typeof strategyTaskStepJsonSchema>,
): StrategyTaskStep => ({
  phaseKey: json.phase_key,
  executionStepId: json.execution_step_id,
  label: json.label,
  model: json.model,
  status: json.status,
  ...(json.item !== undefined ? { item: json.item } : {}),
  ...(json.item_label !== undefined ? { itemLabel: json.item_label } : {}),
  ...(json.output !== undefined ? { output: json.output } : {}),
  startedAt: json.started_at,
  ...(json.finished_at !== undefined ? { finishedAt: json.finished_at } : {}),
  traceId: json.trace_id,
  spanId: json.span_id,
  ...(json.error !== undefined ? { error: json.error } : {}),
})
