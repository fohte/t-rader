import { err, ok, Result } from 'neverthrow'

import { isPlainObject } from '#strategy-agent/agent-graph/json'

export type ExecutionToolRequest =
  | {
      readonly name: 'execute_operation'
      readonly arguments: {
        readonly name: string
        readonly args: Readonly<Record<string, unknown>>
        readonly payload_inputs: readonly {
          readonly field: string
          readonly payload_id: string
          readonly pointer: string
          readonly cursor?: number
          readonly limit?: number
        }[]
      }
    }
  | {
      readonly name: 'read_execution_payload'
      readonly arguments: {
        readonly payload_id: string
        readonly pointer: string
        readonly cursor: number
        readonly limit: number
      }
    }
  | {
      readonly name: 'capture_dataset'
      readonly arguments: {
        readonly provider: string
        readonly params: Readonly<Record<string, unknown>>
      }
    }
  | {
      readonly name: 'read_task_execution_snapshot'
      readonly arguments: Record<string, never>
    }

const parseJson = Result.fromThrowable(
  (text: string): unknown => JSON.parse(text),
  (error): Error =>
    new Error('execution MCP tool returned invalid JSON', { cause: error }),
)

const isTextContentBlock = (
  value: unknown,
): value is { readonly type: 'text'; readonly text: string } =>
  isPlainObject(value) &&
  value['type'] === 'text' &&
  typeof value['text'] === 'string'

export const parseExecutionToolResult = (
  raw: unknown,
): Result<unknown, Error> => {
  if (!isPlainObject(raw)) {
    return err(new Error('execution MCP tool returned a malformed result'))
  }
  if (raw['isError'] === true) {
    return err(new Error('execution MCP tool returned an error'))
  }
  if (isPlainObject(raw['structuredContent'])) {
    return ok(raw['structuredContent'])
  }
  const content = raw['content']
  if (!Array.isArray(content)) {
    return err(new Error('execution MCP tool returned no content'))
  }
  const textBlock = content.find(isTextContentBlock)
  if (textBlock === undefined) {
    return err(new Error('execution MCP tool returned no text content'))
  }
  return parseJson(textBlock.text)
}
