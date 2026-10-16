import { captureWithFingerprint } from '@fohte/service-kit/observability'
import { HumanMessage } from '@langchain/core/messages'
import { DynamicStructuredTool } from '@langchain/core/tools'
import { err, ok, type Result, ResultAsync } from 'neverthrow'
import { z } from 'zod'

import { withPhaseSpan } from '#strategy-agent/agent-graph/tracing'
import type { McpToolsClient } from '#strategy-agent/strategy-agent'
import type { SubagentDefinition } from '#strategy-agent/subagent-definition-client'
import {
  buildSubagentAgent,
  type CreateRunSubagentToolOptions,
  finalizeTimedOutSubagent,
  getParentDeadlineError,
  toError,
} from '#strategy-agent/subagent-timeout'

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
      const executionStepId = crypto.randomUUID()
      const spanName = `Subagent ${key}`
      const result = await withPhaseSpan(
        spanName,
        {
          'phase.key': options.phase.key,
          'phase.execution_step_id': options.parentExecutionStepId,
          'subagent.key': key,
        },
        async (spanIds) => {
          const startStep = (label: string, model: string): number =>
            options.recorder.start({
              phaseKey: `subagent:${key}`,
              executionStepId,
              label,
              model,
              item: { brief },
              startedAt: new Date().toISOString(),
              traceId: spanIds.traceId,
              spanId: spanIds.spanId,
            })
          const fetched = await ResultAsync.fromPromise(
            Promise.resolve().then(() => options.fetchDefinition(key)),
            toError,
          ).andThen((result) => result)
          if (fetched.isErr()) {
            const stepIndex = startStep(key, 'unknown')
            options.recorder.finish(stepIndex, {
              status: 'failed',
              error: fetched.error.message,
            })
            return err(fetched.error)
          }
          const definition = fetched.value
          const stepIndex = startStep(definition.description, definition.model)
          if (definition.key !== key) {
            const error = new Error(
              `subagent definition key mismatch for ${key}`,
            )
            options.recorder.finish(stepIndex, {
              status: 'failed',
              error: error.message,
            })
            return err(error)
          }
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
