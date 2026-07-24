import { errAsync, okAsync, ResultAsync } from 'neverthrow'

export interface AgentConfig {
  readonly agentsMd: string
  readonly skills: Readonly<Record<string, string>>
  readonly model: string
  readonly smallModel: string
}

export class AgentConfigFetchError extends Error {
  constructor(message: string, cause?: unknown) {
    super(message, cause === undefined ? undefined : { cause })
    this.name = 'AgentConfigFetchError'
  }
}

interface AgentConfigResponseBody {
  agents_md: string
  skills: Record<string, string>
  model: string
  small_model: string
}

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
    isRecordOfStrings(record['skills'])
  )
}

export type FetchAgentConfig = (
  strategyId: string,
) => ResultAsync<AgentConfig, AgentConfigFetchError>

export const createAgentConfigFetcher = (
  backendApiBaseUrl: string,
): FetchAgentConfig => {
  return (strategyId) =>
    ResultAsync.fromPromise(
      fetch(`${backendApiBaseUrl}/api/strategies/${strategyId}/agent-config`),
      (error) =>
        new AgentConfigFetchError(
          `failed to fetch agent config for strategy ${strategyId}`,
          error,
        ),
    )
      .andThen((res) => {
        if (!res.ok) {
          return errAsync(
            new AgentConfigFetchError(
              `failed to fetch agent config for strategy ${strategyId}: ${String(res.status)}`,
            ),
          )
        }
        return ResultAsync.fromPromise(
          res.json(),
          (error) =>
            new AgentConfigFetchError(
              `failed to parse agent-config response body for strategy ${strategyId}`,
              error,
            ),
        )
      })
      .andThen((body) => {
        if (!isAgentConfigResponseBody(body)) {
          return errAsync(
            new AgentConfigFetchError(
              `malformed agent-config response for strategy ${strategyId}: expected agents_md/model/small_model strings and a skills map of strings`,
            ),
          )
        }
        return okAsync({
          agentsMd: body.agents_md,
          skills: body.skills,
          model: body.model,
          smallModel: body.small_model,
        })
      })
}
