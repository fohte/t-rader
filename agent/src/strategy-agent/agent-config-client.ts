import { errAsync, okAsync, ResultAsync } from 'neverthrow'

import type { components } from '#lib/api/schema.gen'

export interface AgentConfig {
  readonly agentsMd: string
  readonly skills: Readonly<Record<string, string>>
  readonly model: string
  readonly smallModel: string
  readonly agentGraph: string
}

export class AgentConfigFetchError extends Error {
  constructor(message: string, cause?: unknown) {
    super(message, cause === undefined ? undefined : { cause })
    this.name = 'AgentConfigFetchError'
  }
}

type AgentConfigResponseBody = components['schemas']['AgentConfigResponse']

const isRecordOfStrings = (value: unknown): value is Record<string, string> =>
  typeof value === 'object' &&
  value !== null &&
  !Array.isArray(value) &&
  Object.values(value).every((v) => typeof v === 'string')

// Guards against a malformed/mismatched backend response reaching
// buildSystemPrompt as an unhandled TypeError (e.g. `.trim()` on a
// non-string `agents_md`).
const isAgentConfigResponseBody = (
  value: unknown,
): value is AgentConfigResponseBody => {
  if (typeof value !== 'object' || value === null) return false
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- value is an untyped bag; each field is narrowed immediately below via typeof
  const record = value as Record<string, unknown>
  return (
    typeof record['agents_md'] === 'string' &&
    typeof record['model'] === 'string' &&
    typeof record['small_model'] === 'string' &&
    typeof record['agent_graph'] === 'string' &&
    isRecordOfStrings(record['skills'])
  )
}

export interface AgentConfigKey {
  readonly purpose: string
}

export type FetchAgentConfig = (
  key: AgentConfigKey,
) => ResultAsync<AgentConfig, AgentConfigFetchError>

const agentConfigUrl = (
  backendApiBaseUrl: string,
  key: AgentConfigKey,
): string =>
  `${backendApiBaseUrl}/api/agent-configs/${encodeURIComponent(key.purpose)}/agent-config`

const describeKey = (key: AgentConfigKey): string => `purpose ${key.purpose}`

export const createAgentConfigFetcher = (
  backendApiBaseUrl: string,
): FetchAgentConfig => {
  return (key) => {
    const url = agentConfigUrl(backendApiBaseUrl, key)
    const target = describeKey(key)
    return ResultAsync.fromPromise(
      fetch(url),
      (error) =>
        new AgentConfigFetchError(
          `failed to fetch agent config for ${target}`,
          error,
        ),
    )
      .andThen((res) => {
        if (!res.ok) {
          return errAsync(
            new AgentConfigFetchError(
              `failed to fetch agent config for ${target}: ${String(res.status)}`,
            ),
          )
        }
        return ResultAsync.fromPromise(
          res.json(),
          (error) =>
            new AgentConfigFetchError(
              `failed to parse agent-config response body for ${target}`,
              error,
            ),
        )
      })
      .andThen((body) => {
        if (!isAgentConfigResponseBody(body)) {
          return errAsync(
            new AgentConfigFetchError(
              `malformed agent-config response for ${target}: expected agents_md/model/small_model/agent_graph strings and a skills map of strings`,
            ),
          )
        }
        return okAsync({
          agentsMd: body.agents_md,
          skills: body.skills,
          model: body.model,
          smallModel: body.small_model,
          agentGraph: body.agent_graph,
        })
      })
  }
}
