/**
 * agent_graph (YAML) の 1 フェーズをフォームで編集するための型。
 *
 * backend の `AgentGraphPhase` (backend/src/services/agent_graph.rs) と同じ形だが、
 * `runs` は含めない。`for_each` の有無で実行回数が決まり `runs` は実行時に読み捨てられる
 * ため (agent/src/strategy-agent/agent-graph/types.ts 冒頭コメント参照)、フォームの型としても
 * 前提にしない。
 */
export interface AgentGraphPhaseForm {
  key: string
  label: string
  model: string
  prompt: string
  forEach?: string
  labelField?: string
  maxParallel?: number
  skills: string[]
  tools: string[]
  output: Record<string, unknown>
}
