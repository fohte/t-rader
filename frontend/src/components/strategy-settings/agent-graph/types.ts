/**
 * agent_graph (YAML) の 1 フェーズをフォームで編集するための型。
 *
 * backend の `AgentGraphPhase` と同じ形だが、`runs` は含めない。
 * `for_each` の有無で実行回数が決まり、`runs` は実行時に読み捨てられるため、
 * フォームの型としても前提にしない。
 */
export interface AgentGraphPhaseBaseForm {
  key: string
  label: string
  forEach?: string
  labelField?: string
  maxParallel?: number
  timeoutMinutes?: number
  input?: Record<string, unknown>
  output: Record<string, unknown>
}

export interface LlmAgentGraphPhaseForm extends AgentGraphPhaseBaseForm {
  kind?: 'llm'
  model: string
  prompt: string
  reasoningEffort?: string
  skills: string[]
  tools?: string[]
}

export interface OperationAgentGraphPhaseForm extends AgentGraphPhaseBaseForm {
  kind: 'operation'
  model?: null
  prompt?: never
  skills?: never
  tools?: never
  operation: 'evaluate_saved_code'
  codeId: string
}

export type AgentGraphPhaseForm =
  LlmAgentGraphPhaseForm | OperationAgentGraphPhaseForm
