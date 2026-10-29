interface PriorOutputBinding {
  readonly phase: string
  readonly pointer: string
  readonly cursor?: number
  readonly limit?: number
}

export type InputBinding =
  | { readonly literal: unknown }
  | { readonly priorOutput: PriorOutputBinding }
  | {
      readonly dataset: {
        readonly provider: string
        readonly params: Readonly<Record<string, unknown>>
      }
    }

interface AgentGraphPhaseBase {
  readonly key: string
  readonly label: string
  readonly forEach?: string
  readonly labelField?: string
  readonly maxParallel?: number
  readonly timeoutMinutes?: number
  readonly skills: readonly string[]
  readonly subagents: readonly string[]
  readonly maxSubagentRuns?: number
  // 省略時は全 tool を許可する (単一フェーズの現行挙動と同じ)。
  readonly tools?: readonly string[]
  readonly requiredTools?: readonly string[]
  readonly input?: Readonly<Record<string, InputBinding>>
  readonly output: Readonly<Record<string, unknown>>
}

export interface LlmAgentGraphPhase extends AgentGraphPhaseBase {
  readonly kind?: 'llm'
  readonly model: string
  readonly reasoningEffort?: string
  readonly prompt: string
}

export interface OperationAgentGraphPhase extends AgentGraphPhaseBase {
  readonly kind: 'operation'
  readonly model?: null
  readonly operation: 'evaluate_saved_code'
  readonly codeId: string
}

export type AgentGraphPhase = LlmAgentGraphPhase | OperationAgentGraphPhase

export interface AgentGraphConfig {
  // 全体 timeout は backend が deadline_at に反映し、agent は signal として受け取る。
  readonly timeoutMinutes?: number
  readonly executionLimits?: {
    readonly maxModelCalls?: number
    readonly maxToolCalls?: number
    readonly maxSubagentRuns?: number
  }
  readonly toolLimits?: Readonly<Record<string, number>>
  readonly toolModels?: Readonly<Record<string, string>>
  readonly phases: readonly AgentGraphPhase[]
}
