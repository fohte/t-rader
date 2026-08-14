// `for_each`/`label_field`/`runs` は backend/src/services/agent_graph.rs と
// 同じ "<phase_key>.<field>" / 自由文字列の規約に従う。`runs` と
// `label_field` はここでの実行には関与しないため、zod スキーマでは
// 読み取るがこの型には出さない。
export interface AgentGraphPhase {
  readonly key: string
  readonly label: string
  readonly model: string
  readonly prompt: string
  readonly forEach?: string
  readonly maxParallel?: number
  readonly skills: readonly string[]
  readonly tools: readonly string[]
  readonly output: Readonly<Record<string, unknown>>
}

export interface AgentGraphConfig {
  readonly phases: readonly AgentGraphPhase[]
}
