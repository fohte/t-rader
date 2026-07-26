import { captureWithFingerprint } from '@fohte/service-kit/observability'
import { MultiServerMCPClient } from '@langchain/mcp-adapters'
import { err, errAsync, ok, Result, ResultAsync } from 'neverthrow'

import type { StrategyCandidate } from '@/strategy-resolution/resolve-strategy'

export class StrategyCandidatesParseError extends Error {
  constructor(message: string, cause?: unknown) {
    super(message, cause === undefined ? undefined : { cause })
    this.name = 'StrategyCandidatesParseError'
  }
}

export class StrategyCandidatesFetchError extends Error {
  constructor(message: string, cause?: unknown) {
    super(message, cause === undefined ? undefined : { cause })
    this.name = 'StrategyCandidatesFetchError'
  }
}

const MGMT_MCP_CLIENT_CLOSE_FINGERPRINT =
  'strategy-resolution.mgmt-mcp-client.close-failed'

interface ListStrategiesResponseBody {
  strategies: { strategy_id: string; name: string }[]
}

const isListStrategiesResponseBody = (
  value: unknown,
): value is ListStrategiesResponseBody => {
  if (typeof value !== 'object' || value === null) return false
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- value is an untyped bag; each field is narrowed immediately below via typeof
  const record = value as Record<string, unknown>
  const strategies = record['strategies']
  return (
    Array.isArray(strategies) &&
    strategies.every(
      (s: unknown) =>
        typeof s === 'object' &&
        s !== null &&
        // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- s is an untyped bag; each field is narrowed immediately below via typeof
        typeof (s as Record<string, unknown>)['strategy_id'] === 'string' &&
        // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- s is an untyped bag; each field is narrowed immediately below via typeof
        typeof (s as Record<string, unknown>)['name'] === 'string',
    )
  )
}

const isTextContentBlock = (
  value: unknown,
): value is { type: 'text'; text: string } =>
  typeof value === 'object' &&
  value !== null &&
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- value is an untyped bag; each field is narrowed immediately below via typeof
  (value as Record<string, unknown>)['type'] === 'text' &&
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- value is an untyped bag; each field is narrowed immediately below via typeof
  typeof (value as Record<string, unknown>)['text'] === 'string'

const safeJsonParse = Result.fromThrowable(
  (text: string): unknown => JSON.parse(text),
  (error): StrategyCandidatesParseError =>
    new StrategyCandidatesParseError(
      'list_strategies MCP tool returned invalid JSON',
      error,
    ),
)

// Pure so the response parsing/validation can be unit tested without a live
// MCP server (mirrors agent-config-client.ts's response-shape guard). Takes
// the MCP SDK's CallToolResult.content as `unknown` rather than importing
// its type, since this only needs the single text content block it expects.
export const parseListStrategiesToolResult = (
  content: unknown,
): Result<readonly StrategyCandidate[], StrategyCandidatesParseError> => {
  if (!Array.isArray(content)) {
    return err(
      new StrategyCandidatesParseError(
        'list_strategies MCP tool returned no content',
      ),
    )
  }
  const textBlock = content.find(isTextContentBlock)
  if (textBlock === undefined) {
    return err(
      new StrategyCandidatesParseError(
        'list_strategies MCP tool returned no text content',
      ),
    )
  }
  return safeJsonParse(textBlock.text).andThen((parsed) => {
    if (!isListStrategiesResponseBody(parsed)) {
      return err(
        new StrategyCandidatesParseError('malformed list_strategies response'),
      )
    }
    return ok(
      parsed.strategies.map((s) => ({
        strategyId: s.strategy_id,
        name: s.name,
      })),
    )
  })
}

export type FetchStrategyCandidates = () => ResultAsync<
  readonly StrategyCandidate[],
  StrategyCandidatesFetchError | StrategyCandidatesParseError
>

// MultiServerMCPClient's constructor validates its config synchronously
// (zod parse), so this stays on the Result channel rather than letting a
// bad mgmtMcpUrl throw before any client exists to close.
const buildMgmtClient = Result.fromThrowable(
  (mgmtMcpUrl: string) =>
    new MultiServerMCPClient({ mcpServers: { mgmt: { url: mgmtMcpUrl } } }),
  (error): StrategyCandidatesFetchError =>
    new StrategyCandidatesFetchError(
      'failed to construct mgmt MCP client',
      error,
    ),
)

// Real wiring for production use; executor tests inject a fake
// FetchStrategyCandidates directly instead of exercising this MCP plumbing.
export const createStrategyCandidatesFetcher = (
  mgmtMcpUrl: string,
): FetchStrategyCandidates => {
  return () => {
    const clientResult = buildMgmtClient(mgmtMcpUrl)
    if (clientResult.isErr()) {
      return errAsync(clientResult.error)
    }
    const client = clientResult.value
    const closeClient = (): Promise<void> =>
      client.close().catch((closeError: unknown) => {
        console.error('failed to close mgmt MCP client:', closeError)
        captureWithFingerprint(closeError, MGMT_MCP_CLIENT_CLOSE_FINGERPRINT)
      })

    const fetchCandidates = ResultAsync.fromPromise(
      client.getClient('mgmt'),
      (error) =>
        new StrategyCandidatesFetchError(
          'failed to connect to mgmt MCP server',
          error,
        ),
    )
      .andThen((mcpClient) =>
        mcpClient === undefined
          ? errAsync(
              new StrategyCandidatesFetchError(
                'failed to connect to mgmt MCP server',
              ),
            )
          : ResultAsync.fromPromise(
              mcpClient.callTool({ name: 'list_strategies', arguments: {} }),
              (error) =>
                new StrategyCandidatesFetchError(
                  'list_strategies MCP tool call failed',
                  error,
                ),
            ),
      )
      .andThen((toolResult) =>
        toolResult.isError === true
          ? errAsync(
              new StrategyCandidatesFetchError(
                'list_strategies MCP tool call returned an error',
              ),
            )
          : parseListStrategiesToolResult(toolResult.content),
      )

    // closeClient() itself never rejects, so this finally can't override the
    // result/error already determined above with a rejection of its own.
    return new ResultAsync(
      Promise.resolve(fetchCandidates).finally(() => closeClient()),
    )
  }
}
