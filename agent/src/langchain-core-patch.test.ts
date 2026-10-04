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
const constructors = [AIMessageChunk, CjsAIMessageChunk]

describe('@langchain/core AIMessageChunk patch', () => {
  it('defers tool call parsing and returns the same values in ESM and CJS', () => {
    const actual = constructors.map((MessageChunk) => {
      let elementReads = 0
      const toolCallChunks = new Proxy(
        [
          {
            type: 'tool_call_chunk' as const,
            id: 'call_demo',
            name: 'demo_tool',
            args: '{"label":"demo"}',
            index: 0,
          },
        ],
        {
          get(target, property, receiver) {
            if (typeof property === 'string' && /^\d+$/.test(property)) {
              elementReads += 1
            }
            const value: unknown = Reflect.get(target, property, receiver)
            return value
          },
        },
      )
      const chunk = new MessageChunk({
        content: '',
        tool_call_chunks: toolCallChunks,
      })
      const elementReadsBeforeAccess = elementReads

      return {
        elementReadsBeforeAccess,
        toolCalls: chunk.tool_calls,
        invalidToolCalls: chunk.invalid_tool_calls,
      }
    })

    expect(actual).toEqual([
      {
        elementReadsBeforeAccess: 0,
        toolCalls: [
          {
            type: 'tool_call',
            id: 'call_demo',
            name: 'demo_tool',
            args: { label: 'demo' },
          },
        ],
        invalidToolCalls: [],
      },
      {
        elementReadsBeforeAccess: 0,
        toolCalls: [
          {
            type: 'tool_call',
            id: 'call_demo',
            name: 'demo_tool',
            args: { label: 'demo' },
          },
        ],
        invalidToolCalls: [],
      },
    ])
  })

  it('aggregates valid and invalid streamed tool calls in ESM and CJS', () => {
    const actual = constructors.map((MessageChunk) => {
      const first = new MessageChunk({
        content: '',
        tool_call_chunks: [
          {
            type: 'tool_call_chunk',
            id: 'call_valid',
            name: 'demo_tool',
            args: '{"label":',
            index: 0,
          },
        ],
      })
      const second = new MessageChunk({
        content: '',
        tool_call_chunks: [
          {
            type: 'tool_call_chunk',
            args: '"demo"}',
            index: 0,
          },
          {
            type: 'tool_call_chunk',
            id: 'call_invalid',
            name: 'demo_tool',
            args: 'not-json',
            index: 1,
          },
        ],
      })
      const aggregated = first.concat(second)

      return {
        toolCalls: aggregated.tool_calls,
        invalidToolCalls: aggregated.invalid_tool_calls,
      }
    })

    expect(actual).toEqual([
      {
        toolCalls: [
          {
            type: 'tool_call',
            id: 'call_valid',
            name: 'demo_tool',
            args: { label: 'demo' },
          },
        ],
        invalidToolCalls: [
          {
            type: 'invalid_tool_call',
            id: 'call_invalid',
            name: 'demo_tool',
            args: 'not-json',
            error: 'Malformed args.',
          },
        ],
      },
      {
        toolCalls: [
          {
            type: 'tool_call',
            id: 'call_valid',
            name: 'demo_tool',
            args: { label: 'demo' },
          },
        ],
        invalidToolCalls: [
          {
            type: 'invalid_tool_call',
            id: 'call_invalid',
            name: 'demo_tool',
            args: 'not-json',
            error: 'Malformed args.',
          },
        ],
      },
    ])
  })

  it('serializes collapsed tool call values in ESM and CJS', () => {
    const actual = constructors.map((MessageChunk) => {
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

      return {
        toolCalls: serialized.kwargs['tool_calls'],
        invalidToolCalls: serialized.kwargs['invalid_tool_calls'],
      }
    })

    expect(actual).toEqual([
      {
        toolCalls: [
          {
            type: 'tool_call',
            id: 'call_demo',
            name: 'demo_tool',
            args: { label: 'demo' },
          },
        ],
        invalidToolCalls: [],
      },
      {
        toolCalls: [
          {
            type: 'tool_call',
            id: 'call_demo',
            name: 'demo_tool',
            args: { label: 'demo' },
          },
        ],
        invalidToolCalls: [],
      },
    ])
  })

  it('allows assigning tool_calls after construction in ESM and CJS', () => {
    const actual = constructors.map((MessageChunk) => {
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

      return {
        toolCalls: chunk.tool_calls,
        invalidToolCalls: chunk.invalid_tool_calls,
      }
    })

    expect(actual).toEqual([
      {
        toolCalls: [
          {
            type: 'tool_call',
            id: 'call_assigned',
            name: 'demo_tool',
            args: {},
          },
        ],
        invalidToolCalls: [],
      },
      {
        toolCalls: [
          {
            type: 'tool_call',
            id: 'call_assigned',
            name: 'demo_tool',
            args: {},
          },
        ],
        invalidToolCalls: [],
      },
    ])
  })
})
