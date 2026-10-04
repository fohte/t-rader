import { captureWithFingerprint } from '@fohte/service-kit/observability'
import { MultiServerMCPClient } from '@langchain/mcp-adapters'
import { errAsync, Result, ResultAsync } from 'neverthrow'

import { logger } from '#logger'
import { parseListStrategiesToolResult } from '#strategy-resolution/parse-list-strategies-tool-result'
import type { StrategyCandidate } from '#strategy-resolution/resolve-strategy'

class StrategyCandidatesFetchError extends Error {
  constructor(message: string, cause?: unknown) {
    super(message, cause === undefined ? undefined : { cause })
    this.name = 'StrategyCandidatesFetchError'
  }
}

const MGMT_MCP_CLIENT_CLOSE_FINGERPRINT =
  'strategy-resolution.mgmt-mcp-client.close-failed'

export type FetchStrategyCandidates = () => ResultAsync<
  readonly StrategyCandidate[],
  Error
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
  backendApiBaseUrl: string,
): FetchStrategyCandidates => {
  const mgmtMcpUrl = `${backendApiBaseUrl.replace(/\/+$/, '')}/mcp/mgmt`

  return () => {
    const clientResult = buildMgmtClient(mgmtMcpUrl)
    if (clientResult.isErr()) {
      return errAsync(clientResult.error)
    }
    const client = clientResult.value
    const closeClient = (): Promise<void> =>
      client.close().catch((closeError: unknown) => {
        logger.error({ err: closeError }, 'failed to close mgmt MCP client')
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
