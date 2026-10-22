import type { DynamicStructuredTool } from '@langchain/core/tools'
import { MultiServerMCPClient } from '@langchain/mcp-adapters'
import type { Result } from 'neverthrow'
import { errAsync, ResultAsync } from 'neverthrow'

import type { ExecutionToolRequest } from '#strategy-agent/execution-tool-client'
import { parseExecutionToolResult } from '#strategy-agent/execution-tool-client'

const STRATEGY_ID_HEADER = 'x-strategy-id'
const EXECUTION_ID_HEADER = 'x-execution-id'
const SUBAGENT_EXECUTION_ID_HEADER = 'x-subagent-execution-id'
const SUBAGENT_DEFINITION_KEY_HEADER = 'x-subagent-definition-key'
const TOOL_MODELS_HEADER = 'x-tool-models'
const MCP_TOOL_TIMEOUT_MS = 150_000

export interface McpToolsClient {
  getTools(): Promise<DynamicStructuredTool[]>
  callExecutionTool?(
    request: ExecutionToolRequest,
    signal?: AbortSignal,
  ): Promise<Result<unknown, Error>>
  close(): Promise<void>
}

interface SubagentExecutionIdentity {
  readonly executionId: string
  readonly definitionKey: string
}

export type CreateStrategyMcpClient = (
  strategyId: string,
  executionId: string,
  toolModels: Readonly<Record<string, string>>,
  subagentExecution?: SubagentExecutionIdentity,
) => McpToolsClient

export const createStrategyMcpClient =
  (backendApiBaseUrl: string): CreateStrategyMcpClient =>
  (strategyId, executionId, toolModels, subagentExecution) => {
    const client = new MultiServerMCPClient({
      defaultToolTimeout: MCP_TOOL_TIMEOUT_MS,
      mcpServers: {
        strategy: {
          url: `${backendApiBaseUrl.replace(/\/+$/, '')}/mcp/strategy`,
          headers: {
            [STRATEGY_ID_HEADER]: strategyId,
            [EXECUTION_ID_HEADER]: executionId,
            [TOOL_MODELS_HEADER]: JSON.stringify(toolModels),
            ...(subagentExecution !== undefined
              ? {
                  [SUBAGENT_EXECUTION_ID_HEADER]: subagentExecution.executionId,
                  [SUBAGENT_DEFINITION_KEY_HEADER]:
                    subagentExecution.definitionKey,
                }
              : {}),
          },
        },
      },
    })
    return {
      getTools: () => client.getTools(),
      close: () => client.close(),
      callExecutionTool: async (request, signal) =>
        await ResultAsync.fromPromise(
          client.getClient('strategy'),
          (error) =>
            new Error('failed to connect to strategy MCP server', {
              cause: error,
            }),
        ).andThen((mcpClient) =>
          mcpClient === undefined
            ? errAsync(new Error('failed to connect to strategy MCP server'))
            : ResultAsync.fromPromise(
                mcpClient.callTool(
                  request,
                  undefined,
                  signal === undefined ? undefined : { signal },
                ),
                (error) =>
                  new Error(`${request.name} MCP tool call failed`, {
                    cause: error,
                  }),
              ).andThen(parseExecutionToolResult),
        ),
    }
  }
