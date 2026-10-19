import { ToolMessage } from '@langchain/core/messages'
import type { DynamicStructuredTool, ServerTool } from '@langchain/core/tools'
import { convertToOpenAITool } from '@langchain/core/utils/function_calling'
import { createMiddleware } from 'langchain'
import { err, ok, Result } from 'neverthrow'

import { isPlainObject } from '#strategy-agent/agent-graph/json'

const parseJson = Result.fromThrowable(
  (value: string): unknown => JSON.parse(value),
  (): string => 'args_json must contain valid JSON',
)

const toolNameOf = (tool: unknown): string | undefined =>
  typeof tool === 'object' &&
  tool !== null &&
  'name' in tool &&
  typeof tool.name === 'string'
    ? tool.name
    : undefined

const toEnvelopeTool = (tool: DynamicStructuredTool): ServerTool => {
  const originalSchema = convertToOpenAITool(tool).function.parameters
  const schemaDescription = JSON.stringify(originalSchema)

  return {
    type: 'function',
    function: {
      name: tool.name,
      description: [
        tool.description,
        'Pass the complete tool arguments as a JSON object string in args_json.',
        `Original input JSON Schema: ${schemaDescription}`,
      ].join('\n\n'),
      parameters: {
        type: 'object',
        properties: {
          args_json: {
            type: 'string',
            description:
              'The complete tool arguments encoded as a JSON object.',
          },
        },
        required: ['args_json'],
        additionalProperties: false,
      },
    },
  }
}

const parseEnvelope = (
  args: unknown,
): Result<Record<string, unknown>, string> => {
  if (!isPlainObject(args) || typeof args['args_json'] !== 'string') {
    return err('args_json must be a string containing a JSON object')
  }

  return parseJson(args['args_json']).andThen((parsed) =>
    isPlainObject(parsed)
      ? ok(parsed)
      : err('args_json must contain a JSON object'),
  )
}

export const createMcpToolJsonEnvelopeMiddleware = (
  mcpTools: readonly DynamicStructuredTool[],
) => {
  const mcpToolsByName = new Map(mcpTools.map((tool) => [tool.name, tool]))

  return createMiddleware({
    name: 'mcpToolJsonEnvelopeMiddleware',
    wrapModelCall: async (request, handler) => {
      const tools = request.tools.map((tool) => {
        const name = toolNameOf(tool)
        const mcpTool =
          name === undefined ? undefined : mcpToolsByName.get(name)
        return mcpTool === undefined ? tool : toEnvelopeTool(mcpTool)
      })
      return handler({ ...request, tools })
    },
    wrapToolCall: async (request, handler) => {
      if (!mcpToolsByName.has(request.toolCall.name)) return handler(request)

      const parsedArgs = parseEnvelope(request.toolCall.args)
      if (parsedArgs.isErr()) {
        return new ToolMessage({
          content: `${parsedArgs.error}. Please provide corrected tool arguments.`,
          tool_call_id: request.toolCall.id ?? '',
          name: request.toolCall.name,
          status: 'error',
        })
      }

      return handler({
        ...request,
        toolCall: { ...request.toolCall, args: parsedArgs.value },
      })
    },
  })
}
