import type { BaseChatModel } from '@langchain/core/language_models/chat_models'
import type { HumanMessage } from '@langchain/core/messages'
import type { DynamicStructuredTool } from '@langchain/core/tools'

import type { ObjectJsonSchema } from '#strategy-agent/agent-graph/output-schema'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import type { McpToolsClient } from '#strategy-agent/strategy-agent'

export interface BuildPhaseAgentOptions {
  readonly model: BaseChatModel
  readonly tools: readonly DynamicStructuredTool[]
  readonly systemPrompt: string
  readonly responseSchema: ObjectJsonSchema
}

export interface CompiledPhaseAgent {
  invoke(input: {
    messages: readonly HumanMessage[]
    // 実行ステップの識別子。省略時はステップ単位の識別を行わない。
    executionStepId?: string
  }): Promise<{
    structuredResponse?: Record<string, unknown>
  }>
}

export interface RunAgentGraphDeps {
  readonly buildPhaseAgent: (
    options: BuildPhaseAgentOptions,
  ) => CompiledPhaseAgent
  readonly createChatModel: (
    model: string,
    options?: { reasoningEffort?: string },
  ) => BaseChatModel
}

export interface RunAgentGraphContext {
  readonly agentsMd: string
  readonly skills: Readonly<Record<string, string>>
  // 実行ステップ 1 件だけを担う MCP client を組み立てる。同時に開いている
  // コネクション数を並列度で抑えるため、ステップの終了時に close される。
  readonly createStepMcpClient: (executionStepId: string) => McpToolsClient
  readonly originalPromptText: string
  // 実行中のフェーズ/for_each 要素ごとの進捗を都度通知する。呼び出し側は
  // 受け取った配列全体を steps の最新状態として扱う (差分ではない)。
  readonly onStepsChanged?: (steps: readonly StrategyTaskStep[]) => void
  // resume 対象タスクの全ステップ (backend の全 strategy_task_step 行)。
  // status='completed' のものだけスキップ対象になり、それ以外 (failed/running)
  // は元の executionStepId を再利用して再実行する。
  readonly previousSteps?: readonly StrategyTaskStep[]
}

export type StepStartInput = Omit<
  StrategyTaskStep,
  'status' | 'finishedAt' | 'output' | 'error'
>
export type StepOutcome =
  | { readonly status: 'completed'; readonly output: unknown }
  | { readonly status: 'failed'; readonly error: string }

export interface StepRecorder {
  readonly start: (step: StepStartInput) => number
  readonly finish: (index: number, outcome: StepOutcome) => void
  // resume でスキップした (再実行しない) 完了済みステップをそのまま steps に
  // 積む。start/finish を経由しないので running を経由せず直接完了状態になる。
  readonly recordExisting: (step: StrategyTaskStep) => void
}
