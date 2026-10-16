import { errAsync, okAsync, ResultAsync } from 'neverthrow'

import type { components } from '#lib/api/schema.gen'
import { isPlainObject } from '#strategy-agent/agent-graph/json'

type SubagentDefinitionResponse =
  components['schemas']['SubagentDefinitionResponse']

export interface SubagentDefinition {
  readonly key: string
  readonly description: string
  readonly model: string
  readonly reasoningEffort?: string
  readonly prompt: string
  readonly tools: readonly string[]
  readonly timeoutMinutes: number
  readonly maxModelCalls: number
  readonly toolLimits: Readonly<Record<string, number>>
  readonly output: Readonly<Record<string, unknown>>
}

class SubagentDefinitionFetchError extends Error {
  constructor(message: string, cause?: unknown) {
    super(message, cause === undefined ? undefined : { cause })
    this.name = 'SubagentDefinitionFetchError'
  }
}

const isRecordOfNumbers = (value: unknown): value is Record<string, number> =>
  isPlainObject(value) &&
  Object.values(value).every((item) => typeof item === 'number')

const isSubagentDefinitionResponse = (
  value: unknown,
): value is SubagentDefinitionResponse => {
  if (!isPlainObject(value)) return false
  const record = value
  return (
    typeof record['id'] === 'string' &&
    typeof record['key'] === 'string' &&
    typeof record['description'] === 'string' &&
    typeof record['model'] === 'string' &&
    (record['reasoning_effort'] === undefined ||
      record['reasoning_effort'] === null ||
      typeof record['reasoning_effort'] === 'string') &&
    typeof record['prompt'] === 'string' &&
    Array.isArray(record['tools']) &&
    record['tools'].every((tool) => typeof tool === 'string') &&
    typeof record['timeout_minutes'] === 'number' &&
    typeof record['max_model_calls'] === 'number' &&
    isRecordOfNumbers(record['tool_limits']) &&
    isPlainObject(record['output']) &&
    typeof record['created_at'] === 'string' &&
    typeof record['updated_at'] === 'string'
  )
}

export type FetchSubagentDefinition = (
  key: string,
) => ResultAsync<SubagentDefinition, Error>

export const createSubagentDefinitionFetcher = (
  backendApiBaseUrl: string,
): FetchSubagentDefinition => {
  return (key) => {
    const url = `${backendApiBaseUrl.replace(/\/+$/, '')}/api/subagent-definitions/${encodeURIComponent(key)}`
    return ResultAsync.fromPromise(
      fetch(url),
      (error) =>
        new SubagentDefinitionFetchError(
          `failed to fetch subagent definition for key ${key}`,
          error,
        ),
    )
      .andThen((res) => {
        if (!res.ok) {
          return errAsync(
            new SubagentDefinitionFetchError(
              `failed to fetch subagent definition for key ${key}: ${String(res.status)}`,
            ),
          )
        }
        return ResultAsync.fromPromise(
          res.json(),
          (error) =>
            new SubagentDefinitionFetchError(
              `failed to parse subagent-definition response body for key ${key}`,
              error,
            ),
        )
      })
      .andThen((body) => {
        if (!isSubagentDefinitionResponse(body)) {
          return errAsync(
            new SubagentDefinitionFetchError(
              `malformed subagent-definition response for key ${key}`,
            ),
          )
        }
        return okAsync({
          key: body.key,
          description: body.description,
          model: body.model,
          ...(body.reasoning_effort != null
            ? { reasoningEffort: body.reasoning_effort }
            : {}),
          prompt: body.prompt,
          tools: body.tools,
          timeoutMinutes: body.timeout_minutes,
          maxModelCalls: body.max_model_calls,
          toolLimits: body.tool_limits,
          output: body.output,
        })
      })
  }
}
