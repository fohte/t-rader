import { createRequire } from 'node:module'

import { AIMessageChunk } from '@langchain/core/messages'
import { describe, expect, it } from 'vitest'
import { z } from 'zod'

const require = createRequire(import.meta.url)
const cjsExports: unknown = require('@langchain/core/messages')

const CjsAIMessageChunk = z
  .object({
    AIMessageChunk: z.custom<typeof AIMessageChunk>(
      (value) => typeof value === 'function',
    ),
  })
  .parse(cjsExports).AIMessageChunk

const messageChunkFormats = [
  { format: 'ESM', MessageChunk: AIMessageChunk },
  { format: 'CJS', MessageChunk: CjsAIMessageChunk },
] as const

function countReductions<T>(values: T[], onReduce: () => void): T[] {
  const originalReduce = values.reduce.bind(values)
  Object.defineProperty(values, 'reduce', {
    value: (...args: Parameters<typeof originalReduce>) => {
      onReduce()
      return originalReduce(...args)
    },
  })
  return values
}

describe.each(messageChunkFormats)(
  '@langchain/core AIMessageChunk patch ($format)',
  ({ MessageChunk }) => {
    it('defers tool call parsing through concatenation and caches the result', () => {
      let collapseCount = 0

      class CountingMessageChunk extends MessageChunk {
        constructor(fields: ConstructorParameters<typeof MessageChunk>[0]) {
          const trackedFields =
            typeof fields === 'string' ||
            Array.isArray(fields) ||
            fields.tool_call_chunks === undefined
              ? fields
              : {
                  ...fields,
                  tool_call_chunks: countReductions(
                    fields.tool_call_chunks,
                    () => {
                      collapseCount += 1
                    },
                  ),
                }
          super(trackedFields)
        }
      }

      let aggregated: InstanceType<typeof MessageChunk> =
        new CountingMessageChunk({
          content: '',
          tool_call_chunks: [
            {
              type: 'tool_call_chunk',
              id: 'call_valid',
              name: 'demo_tool',
              args: '{"label":"',
              index: 0,
            },
          ],
        })

      for (const chunk of [
        new CountingMessageChunk({
          content: '',
          tool_call_chunks: [
            { type: 'tool_call_chunk', args: 'demo', index: 0 },
          ],
        }),
        new CountingMessageChunk({
          content: '',
          tool_call_chunks: [{ type: 'tool_call_chunk', args: '"}', index: 0 }],
        }),
        new CountingMessageChunk({
          content: '',
          tool_call_chunks: [
            {
              type: 'tool_call_chunk',
              id: 'call_invalid',
              name: 'demo_tool',
              args: 'not-json',
              index: 1,
            },
          ],
        }),
      ]) {
        aggregated = aggregated.concat(chunk)
      }

      const collapseCountBeforeAccess = collapseCount
      const toolCalls = aggregated.tool_calls
      const invalidToolCalls = aggregated.invalid_tool_calls
      const repeatedToolCalls = aggregated.tool_calls
      const repeatedInvalidToolCalls = aggregated.invalid_tool_calls

      expect(
        Object.freeze([
          collapseCountBeforeAccess,
          collapseCount,
          toolCalls === repeatedToolCalls,
          invalidToolCalls === repeatedInvalidToolCalls,
          toolCalls,
          invalidToolCalls,
        ]),
      ).toEqual([
        0,
        1,
        true,
        true,
        [
          {
            type: 'tool_call',
            id: 'call_valid',
            name: 'demo_tool',
            args: { label: 'demo' },
          },
        ],
        [
          {
            type: 'invalid_tool_call',
            id: 'call_invalid',
            name: 'demo_tool',
            args: 'not-json',
            error: 'Malformed args.',
          },
        ],
      ])
    })

    it('serializes collapsed tool call values', () => {
      const chunk = new MessageChunk({
        content: '',
        tool_call_chunks: [
          {
            type: 'tool_call_chunk',
            id: 'call_demo',
            name: 'demo_tool',
            args: '{"label":"demo"}',
            index: 0,
          },
        ],
      })
      const serializedJson: unknown = JSON.parse(JSON.stringify(chunk))
      const serialized = z
        .object({ kwargs: z.record(z.string(), z.unknown()) })
        .parse(serializedJson)

      expect(
        Object.freeze([
          serialized.kwargs['tool_calls'],
          serialized.kwargs['invalid_tool_calls'],
        ]),
      ).toEqual([
        [
          {
            type: 'tool_call',
            id: 'call_demo',
            name: 'demo_tool',
            args: { label: 'demo' },
          },
        ],
        [],
      ])
    })

    it('allows assigning tool_calls after construction', () => {
      const chunk = new MessageChunk({
        content: '',
        tool_call_chunks: [
          {
            type: 'tool_call_chunk',
            id: 'call_initial',
            name: 'demo_tool',
            args: '{"label":"initial"}',
            index: 0,
          },
        ],
      })
      chunk.tool_calls = [
        { type: 'tool_call', id: 'call_assigned', name: 'demo_tool', args: {} },
      ]

      expect(
        Object.freeze([chunk.tool_calls, chunk.invalid_tool_calls]),
      ).toEqual([
        [
          {
            type: 'tool_call',
            id: 'call_assigned',
            name: 'demo_tool',
            args: {},
          },
        ],
        [],
      ])
    })
  },
)
