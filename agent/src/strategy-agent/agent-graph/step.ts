import { z } from 'zod'

// agent_graph の 1 フェーズ (for_each の場合は要素 1 件) の実行状況。backend の
// strategy_task.steps (jsonb, フラット配列) にそのまま乗る契約 (backend は中身を
// 解釈せず素通しする) なので、キー名は他の内部 API レスポンスと同じ snake_case にする。
export const AGENT_GRAPH_STEPS_ARTIFACT_ID = 'agent-graph-steps'

type StrategyTaskStepStatus = 'running' | 'completed' | 'failed'
type StrategyTaskStepKind = 'llm' | 'operation'

export interface StrategyTaskStep {
  readonly phaseKey: string
  readonly executionStepId: string
  readonly kind?: StrategyTaskStepKind
  readonly label: string
  readonly model: string | null
  readonly status: StrategyTaskStepStatus
  readonly item?: unknown
  readonly itemKey?: unknown
  readonly itemLabel?: string
  readonly inputRef?: unknown
  readonly outputRef?: unknown
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
  readonly kind?: StrategyTaskStepKind
  readonly label: string
  readonly model: string | null
  readonly status: StrategyTaskStepStatus
  readonly item?: unknown
  readonly item_key?: unknown
  readonly item_label?: string
  readonly input_ref?: unknown
  readonly output_ref?: unknown
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
  ...(step.kind !== undefined ? { kind: step.kind } : {}),
  label: step.label,
  model: step.model,
  status: step.status,
  ...(step.item !== undefined ? { item: step.item } : {}),
  ...(step.itemKey !== undefined ? { item_key: step.itemKey } : {}),
  ...(step.itemLabel !== undefined ? { item_label: step.itemLabel } : {}),
  ...(step.inputRef !== undefined ? { input_ref: step.inputRef } : {}),
  ...(step.outputRef !== undefined ? { output_ref: step.outputRef } : {}),
  ...(step.output !== undefined ? { output: step.output } : {}),
  started_at: step.startedAt,
  ...(step.finishedAt !== undefined ? { finished_at: step.finishedAt } : {}),
  trace_id: step.traceId,
  span_id: step.spanId,
  ...(step.error !== undefined ? { error: step.error } : {}),
})

// resume 用に backend (`POST /internal/tasks` の `resume_steps`) から届く wire JSON
// の形状。中身は backend 側 (step_to_resume_wire_json) の契約で保証されている前提だが、
// 型システムの外から届く値のため実行時にも検証する。
export const strategyTaskStepJsonSchema = z.object({
  phase_key: z.string(),
  execution_step_id: z.string(),
  kind: z.enum(['llm', 'operation']).optional(),
  label: z.string(),
  model: z.string().nullable(),
  status: z.enum(['running', 'completed', 'failed']),
  item: z.unknown().optional(),
  item_key: z.unknown().optional(),
  item_label: z.string().optional(),
  input_ref: z.unknown().optional(),
  output_ref: z.unknown().optional(),
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
  ...(json.kind !== undefined ? { kind: json.kind } : {}),
  label: json.label,
  model: json.model,
  status: json.status,
  ...(json.item !== undefined ? { item: json.item } : {}),
  ...(json.item_key !== undefined ? { itemKey: json.item_key } : {}),
  ...(json.item_label !== undefined ? { itemLabel: json.item_label } : {}),
  ...(json.input_ref !== undefined ? { inputRef: json.input_ref } : {}),
  ...(json.output_ref !== undefined ? { outputRef: json.output_ref } : {}),
  ...(json.output !== undefined ? { output: json.output } : {}),
  startedAt: json.started_at,
  ...(json.finished_at !== undefined ? { finishedAt: json.finished_at } : {}),
  traceId: json.trace_id,
  spanId: json.span_id,
  ...(json.error !== undefined ? { error: json.error } : {}),
})
