import { captureWithFingerprint } from '@fohte/service-kit/observability'
import { type BaseMessage, HumanMessage } from '@langchain/core/messages'
import { DynamicStructuredTool } from '@langchain/core/tools'
import { err, ok, type Result } from 'neverthrow'
import { z } from 'zod'

import { buildOutputJsonSchema } from '#strategy-agent/agent-graph/output-schema'
import type {
  CompiledPhaseAgent,
  RunAgentGraphDeps,
  StepRecorder,
} from '#strategy-agent/agent-graph/run-agent-graph/types'
import { withPhaseSpan } from '#strategy-agent/agent-graph/tracing'
import type { AgentGraphPhase } from '#strategy-agent/agent-graph/types'
import {
  isPhaseTimeoutSignal,
  type PhaseTimeout,
  phaseTimeoutMessage,
} from '#strategy-agent/phase-timeout'
import type { McpToolsClient } from '#strategy-agent/strategy-agent'
import type {
  FetchSubagentDefinition,
  SubagentDefinition,
} from '#strategy-agent/subagent-definition-client'

const RUN_SUBAGENT_TOOL_NAME = 'run_subagent'
const RUN_SUBAGENT_INPUT_SCHEMA = z.object({
  key: z.string().describe('The configured subagent key.'),
  brief: z
    .string()
    .min(1)
    .describe('The objective and known facts for this run.'),
})
type RunSubagentInput = z.infer<typeof RUN_SUBAGENT_INPUT_SCHEMA>
type RunSubagentToolOutput = Record<string, unknown>
const SUBAGENT_MCP_CLIENT_CLOSE_FAILED_FINGERPRINT =
  'subagent-tool.mcp-client-close-failed'

interface SubagentRunCounter {
  count: number
}

interface CreateRunSubagentToolOptions {
  readonly phase: AgentGraphPhase
  readonly parentExecutionStepId: string
  readonly recorder: StepRecorder
  readonly runCounter: SubagentRunCounter
  readonly fetchDefinition: FetchSubagentDefinition
  readonly createMcpClient: (
    executionStepId: string,
    definitionKey: string,
  ) => McpToolsClient
  readonly deps: RunAgentGraphDeps
  readonly deadlineSignal?: AbortSignal
  readonly phaseTimeout?: PhaseTimeout
}

const toError = (error: unknown): Error =>
  error instanceof Error ? error : new Error(String(error))

const createChildDeadline = (
  parentSignal: AbortSignal | undefined,
  timeoutMinutes: number,
): {
  readonly signal: AbortSignal
  readonly timeoutSignal: AbortSignal
  readonly cleanup: () => void
} => {
  const timeoutController = new AbortController()
  const timeoutId = setTimeout(() => {
    timeoutController.abort()
  }, timeoutMinutes * 60_000)
  return {
    signal:
      parentSignal === undefined
        ? timeoutController.signal
        : AbortSignal.any([parentSignal, timeoutController.signal]),
    timeoutSignal: timeoutController.signal,
    cleanup: () => {
      clearTimeout(timeoutId)
    },
  }
}

const getStructuredResponse = (
  agent: CompiledPhaseAgent,
  messages: readonly BaseMessage[],
  executionStepId: string,
): Promise<Record<string, unknown>> =>
  agent.invoke({ messages, executionStepId }).then((result) => {
    if (result.structuredResponse === undefined) {
      return Promise.reject(
        new Error('subagent returned no structured response'),
      )
    }
    return result.structuredResponse
  })

const waitForAbort = <T>(
  promise: Promise<T>,
  signal: AbortSignal,
): Promise<T> =>
  new Promise<T>((resolve, reject) => {
    const cleanup = (): void => {
      signal.removeEventListener('abort', rejectOnAbort)
    }
    const rejectOnAbort = (): void => {
      cleanup()
      reject(toError(signal.reason ?? new Error('subagent deadline exceeded')))
    }
    if (signal.aborted) {
      rejectOnAbort()
      return
    }
    signal.addEventListener('abort', rejectOnAbort, { once: true })
    promise.then(
      (value) => {
        cleanup()
        resolve(value)
      },
      (error: unknown) => {
        cleanup()
        reject(toError(error))
      },
    )
  })

const buildSubagentAgent = (
  definition: SubagentDefinition,
  deps: RunAgentGraphDeps,
  tools: readonly DynamicStructuredTool[],
  options: {
    readonly deadlineSignal?: AbortSignal
    readonly phaseTimeout?: PhaseTimeout
    readonly modelCallBudget: {
      readonly maxModelCalls: number
      count: number
    }
    readonly systemPrompt?: string
  },
): CompiledPhaseAgent =>
  deps.buildPhaseAgent({
    model: deps.createChatModel(definition.model, {
      ...(definition.reasoningEffort !== undefined
        ? { reasoningEffort: definition.reasoningEffort }
        : {}),
    }),
    tools,
    systemPrompt: options.systemPrompt ?? definition.prompt,
    responseSchema: buildOutputJsonSchema(definition.output),
    maxModelCalls: definition.maxModelCalls,
    modelCallBudget: options.modelCallBudget,
    captureMessagesOnAbort: true,
    ...(options.deadlineSignal !== undefined
      ? { deadlineSignal: options.deadlineSignal }
      : {}),
    ...(options.phaseTimeout !== undefined
      ? { phaseTimeout: options.phaseTimeout }
      : {}),
  })

const getParentDeadlineError = (
  options: CreateRunSubagentToolOptions,
): Error => {
  const message = isPhaseTimeoutSignal(
    options.deadlineSignal,
    options.phaseTimeout,
  )
    ? phaseTimeoutMessage(options.phaseTimeout)
    : 'parent execution deadline exceeded'
  return new Error(message)
}

const finalizeTimedOutSubagent = (
  definition: SubagentDefinition,
  executionStepId: string,
  options: CreateRunSubagentToolOptions,
  modelCallBudget: { readonly maxModelCalls: number; count: number },
  messages: readonly BaseMessage[],
): Promise<Result<Record<string, unknown>, Error>> => {
  if (options.deadlineSignal?.aborted === true) {
    return Promise.resolve(err(getParentDeadlineError(options)))
  }
  if (modelCallBudget.count >= modelCallBudget.maxModelCalls) {
    return Promise.resolve(
      err(
        new Error(
          `subagent ${definition.key} reached its time limit after using all model calls`,
        ),
      ),
    )
  }
  // 調査は止め、親 deadline と子の呼び出し予算を共有したまま結果だけをまとめる。
  const finalizer = buildSubagentAgent(definition, options.deps, [], {
    ...(options.deadlineSignal !== undefined
      ? { deadlineSignal: options.deadlineSignal }
      : {}),
    ...(options.phaseTimeout !== undefined
      ? { phaseTimeout: options.phaseTimeout }
      : {}),
    modelCallBudget,
    systemPrompt: [
      definition.prompt,
      '制限時間に達しました。これまでに得た情報だけを使って結果を返してください。',
    ].join('\n\n'),
  })
  return getStructuredResponse(
    finalizer,
    [
      ...messages,
      new HumanMessage(
        '制限時間に達しました。追加調査は行わず、ここまでの情報から指定形式の結果を返してください。',
      ),
    ],
    executionStepId,
  ).then(
    (response) => ok(response),
    (error: unknown) => err(toError(error)),
  )
}

const runWithDefinition = (
  definition: SubagentDefinition,
  brief: string,
  executionStepId: string,
  options: CreateRunSubagentToolOptions,
): Promise<Result<Record<string, unknown>, Error>> => {
  const childDeadline = createChildDeadline(
    options.deadlineSignal,
    definition.timeoutMinutes,
  )
  const modelCallBudget = {
    maxModelCalls: definition.maxModelCalls,
    count: 0,
  }
  let mcpClient: McpToolsClient | undefined

  const run = Promise.resolve()
    .then(() => options.createMcpClient(executionStepId, definition.key))
    .then((client) => {
      mcpClient = client
      return waitForAbort(client.getTools(), childDeadline.signal)
    })
    .then((availableTools) => {
      const tools = availableTools.filter(
        (tool) =>
          tool.name !== RUN_SUBAGENT_TOOL_NAME &&
          definition.tools.includes(tool.name),
      )
      const agent = buildSubagentAgent(definition, options.deps, tools, {
        deadlineSignal: childDeadline.signal,
        phaseTimeout: {
          signal: childDeadline.timeoutSignal,
          phaseKey: `subagent:${definition.key}`,
          timeoutMinutes: definition.timeoutMinutes,
        },
        modelCallBudget,
      })
      return agent.invoke({
        messages: [new HumanMessage(brief)],
        executionStepId,
      })
    })
    .then((result) => {
      if (result.structuredResponse !== undefined) {
        return ok(result.structuredResponse)
      }
      if (options.deadlineSignal?.aborted === true) {
        return err(getParentDeadlineError(options))
      }
      if (!childDeadline.timeoutSignal.aborted) {
        return err(new Error('subagent returned no structured response'))
      }
      return finalizeTimedOutSubagent(
        definition,
        executionStepId,
        options,
        modelCallBudget,
        result.messages ?? [new HumanMessage(brief)],
      )
    })
    .catch((error: unknown) => {
      if (options.deadlineSignal?.aborted === true) {
        return err(getParentDeadlineError(options))
      }
      if (childDeadline.timeoutSignal.aborted) {
        return finalizeTimedOutSubagent(
          definition,
          executionStepId,
          options,
          modelCallBudget,
          [new HumanMessage(brief)],
        )
      }
      return err(toError(error))
    })
    .finally(() => {
      childDeadline.cleanup()
      if (mcpClient === undefined) return
      return mcpClient.close().catch((error: unknown) => {
        captureWithFingerprint(
          toError(error),
          SUBAGENT_MCP_CLIENT_CLOSE_FAILED_FINGERPRINT,
          { extras: { executionStepId } },
        )
      })
    })

  return run
}

export const createRunSubagentTool = (
  options: CreateRunSubagentToolOptions,
): DynamicStructuredTool<
  typeof RUN_SUBAGENT_INPUT_SCHEMA,
  RunSubagentInput,
  z.input<typeof RUN_SUBAGENT_INPUT_SCHEMA>,
  RunSubagentToolOutput
> =>
  new DynamicStructuredTool<
    typeof RUN_SUBAGENT_INPUT_SCHEMA,
    RunSubagentInput,
    z.input<typeof RUN_SUBAGENT_INPUT_SCHEMA>,
    RunSubagentToolOutput
  >({
    name: RUN_SUBAGENT_TOOL_NAME,
    description: `Run a configured subagent with only the provided brief. Available keys: ${options.phase.subagents.join(', ')}.`,
    schema: RUN_SUBAGENT_INPUT_SCHEMA,
    func: async ({ key, brief }) => {
      if (!options.phase.subagents.includes(key)) {
        return Promise.reject(
          new Error(
            `subagent key ${key} is not allowed in phase ${options.phase.key}`,
          ),
        )
      }
      if (
        options.phase.maxSubagentRuns !== undefined &&
        options.runCounter.count >= options.phase.maxSubagentRuns
      ) {
        return Promise.reject(
          new Error(
            `phase ${options.phase.key} exceeded max_subagent_runs ${String(options.phase.maxSubagentRuns)}`,
          ),
        )
      }
      options.runCounter.count += 1
      const fetched = await options.fetchDefinition(key)
      if (fetched.isErr()) return Promise.reject(fetched.error)
      const definition = fetched.value
      if (definition.key !== key) {
        return Promise.reject(
          new Error(`subagent definition key mismatch for ${key}`),
        )
      }

      const executionStepId = crypto.randomUUID()
      const spanName = `Subagent ${definition.key}`
      const result = await withPhaseSpan(
        spanName,
        {
          'phase.key': options.phase.key,
          'phase.execution_step_id': options.parentExecutionStepId,
          'subagent.key': definition.key,
          'subagent.model': definition.model,
        },
        async (spanIds) => {
          const stepIndex = options.recorder.start({
            phaseKey: `subagent:${definition.key}`,
            executionStepId,
            label: definition.description,
            model: definition.model,
            item: { brief },
            startedAt: new Date().toISOString(),
            traceId: spanIds.traceId,
            spanId: spanIds.spanId,
          })
          const output = await runWithDefinition(
            definition,
            brief,
            executionStepId,
            options,
          )
          options.recorder.finish(
            stepIndex,
            output.match(
              (value) => ({ status: 'completed' as const, output: value }),
              (error) => ({ status: 'failed' as const, error: error.message }),
            ),
          )
          return output
        },
      )
      return result.match(
        (value) => value,
        (error) => Promise.reject(error),
      )
    },
  })
