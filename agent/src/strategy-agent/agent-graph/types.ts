// `for_each`/`label_field`/`runs` follow the same "<phase_key>.<field>" /
// free-string conventions as backend/src/services/agent_graph.rs. `runs` and
// `label_field` carry no execution semantics here (label_field is for the
// execution-view UI in a later PR), so they're read by the zod schema but
// not surfaced on this type.
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
