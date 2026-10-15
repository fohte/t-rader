// `for_each`/`label_field` は backend/crates/core/application/src/agent_config/graph.rs と同じ
// "<phase_key>.<field>" / 自由文字列の規約に従う。
export interface AgentGraphPhase {
  readonly key: string
  readonly label: string
  readonly model: string
  readonly reasoningEffort?: string
  readonly prompt: string
  readonly forEach?: string
  readonly labelField?: string
  readonly maxParallel?: number
  readonly timeoutMinutes?: number
  readonly skills: readonly string[]
  readonly subagents?: readonly string[]
  readonly maxSubagentRuns?: number
  // 省略時は全 tool を許可する (単一フェーズの現行挙動と同じ)。
  readonly tools?: readonly string[]
  readonly output: Readonly<Record<string, unknown>>
}

export interface AgentGraphConfig {
  // 全体 timeout は backend が deadline_at に反映し、agent は signal として受け取る。
  readonly timeoutMinutes?: number
  readonly toolModels?: Readonly<Record<string, string>>
  readonly phases: readonly AgentGraphPhase[]
}
