import { HumanMessage, ToolMessage } from '@langchain/core/messages'
import { DynamicStructuredTool } from '@langchain/core/tools'
import { ChatOpenAI } from '@langchain/openai'
import { createAgent } from 'langchain'
import { describe, expect, it, vi } from 'vitest'
import { z } from 'zod'

import { createStrategyToolErrorMiddleware } from '#strategy-agent/tool-error-middleware'

type CaptureWithFingerprintMock = (
  error: unknown,
  fingerprint: string | readonly string[],
  context?: {
    readonly level?: string
    readonly extras?: Readonly<Record<string, unknown>>
  },
) => void

const { captureWithFingerprintMock } = vi.hoisted(() => ({
  captureWithFingerprintMock: vi.fn<CaptureWithFingerprintMock>(),
}))

vi.mock('@fohte/service-kit/observability', () => ({
  captureWithFingerprint: captureWithFingerprintMock,
}))

type ChatOpenAIFetch = NonNullable<
  NonNullable<ConstructorParameters<typeof ChatOpenAI>[0]>['configuration']
>['fetch']

const buildStubModel = (fetch: ChatOpenAIFetch): ChatOpenAI =>
  new ChatOpenAI({
    apiKey: 'test-key',
    model: 'example-model-test-tool-errors',
    maxRetries: 0,
    configuration: { baseURL: 'http://localhost', fetch },
  })

const buildToolCallResponse = (): Response =>
  new Response(
    JSON.stringify({
      id: 'call-1',
      model: 'example-model-test-tool-errors',
      choices: [
        {
          index: 0,
          finish_reason: 'tool_calls',
          message: {
            role: 'assistant',
            content: null,
            tool_calls: [
              {
                id: 'call-1',
                type: 'function',
                function: { name: 'demo_lookup', arguments: '{}' },
              },
            ],
          },
        },
      ],
    }),
    { status: 200, headers: { 'content-type': 'application/json' } },
  )

const buildStopResponse = (): Response =>
  new Response(
    JSON.stringify({
      id: 'call-2',
      model: 'example-model-test-tool-errors',
      choices: [
        {
          index: 0,
          finish_reason: 'stop',
          message: { role: 'assistant', content: 'done' },
        },
      ],
    }),
    { status: 200, headers: { 'content-type': 'application/json' } },
  )

const normalizeCaptureCalls = () =>
  captureWithFingerprintMock.mock.calls.map(
    ([error, fingerprint, context]) => ({
      errorName: error instanceof Error ? error.name : typeof error,
      errorMessage: error instanceof Error ? error.message : String(error),
      fingerprint:
        typeof fingerprint === 'string' ? [fingerprint] : [...fingerprint],
      level: context?.level ?? null,
      extras: context?.extras ?? null,
    }),
  )

const runFailingTool = async (options: {
  readonly toolError: Error
  readonly toolSchema?: z.ZodType
}) => {
  let modelCallCount = 0
  const model = buildStubModel(() => {
    modelCallCount += 1
    return Promise.resolve(
      modelCallCount === 1 ? buildToolCallResponse() : buildStopResponse(),
    )
  })
  const tool = new DynamicStructuredTool({
    name: 'demo_lookup',
    description: 'Returns a demo result',
    schema: options.toolSchema ?? z.object({}),
    func: () => Promise.reject(options.toolError),
  })
  const agent = createAgent({
    model,
    tools: [tool],
    middleware: [createStrategyToolErrorMiddleware()],
  })
  const result = await agent.invoke({ messages: [new HumanMessage('hi')] })

  return {
    toolMessages: result.messages
      .filter((message) => ToolMessage.isInstance(message))
      .map((message) => message.text),
    captures: normalizeCaptureCalls(),
  }
}

describe('createStrategyToolErrorMiddleware', () => {
  it.each([
    {
      name: 'HTTP transport errors',
      message:
        'Error calling tool demo_lookup: Error POSTing to endpoint (HTTP 503): service unavailable',
    },
    {
      name: 'missing MCP sessions',
      message:
        'Error calling tool demo_lookup: Streamable HTTP error: Error POSTing to endpoint: Session not found',
    },
    {
      name: 'MCP internal errors',
      message:
        'Error calling tool demo_lookup: MCP error -32603: Internal error',
    },
  ])('captures $name with a tool-specific fingerprint', async ({ message }) => {
    captureWithFingerprintMock.mockClear()
    const toolError = new Error(message)
    toolError.name = 'ToolException'

    const result = await runFailingTool({ toolError })

    expect(result).toEqual({
      toolMessages: [`ToolException: ${message}\n Please fix your mistakes.`],
      captures: [
        {
          errorName: 'ToolException',
          errorMessage: message,
          fingerprint: [
            'strategy-agent.mcp-tool-error',
            'demo_lookup',
            '{{ default }}',
          ],
          level: 'error',
          extras: { toolName: 'demo_lookup' },
        },
      ],
    })
  })

  it('does not capture tool input validation failures', async () => {
    captureWithFingerprintMock.mockClear()

    const result = await runFailingTool({
      toolError: new Error('tool implementation should not run'),
      toolSchema: z.object({ query: z.string() }),
    })

    expect(result.captures).toEqual([])
  })

  it('does not capture MCP tool business errors', async () => {
    captureWithFingerprintMock.mockClear()
    const message =
      "MCP tool 'demo_lookup' on server 'strategy' returned an error: quote unavailable"
    const toolError = new Error(message)
    toolError.name = 'ToolException'

    const result = await runFailingTool({ toolError })

    expect(result).toEqual({
      toolMessages: [`ToolException: ${message}\n Please fix your mistakes.`],
      captures: [],
    })
  })
})
