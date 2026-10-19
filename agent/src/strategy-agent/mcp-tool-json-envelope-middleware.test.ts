import {
  type BaseMessage,
  HumanMessage,
  ToolMessage,
} from '@langchain/core/messages'
import { DynamicStructuredTool } from '@langchain/core/tools'
import { convertToOpenAITool } from '@langchain/core/utils/function_calling'
import { createAgent } from 'langchain'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { z } from 'zod'

import { createMcpToolJsonEnvelopeMiddleware } from '#strategy-agent/mcp-tool-json-envelope-middleware'
import type { ChatOpenAIFetch } from '#strategy-agent/reasoning-content-middleware'
import { createStrategyToolErrorMiddleware } from '#strategy-agent/tool-error-middleware'
import { createStubChatModel } from '#test/stub-chat-model'

const { captureWithFingerprintMock } = vi.hoisted(() => ({
  captureWithFingerprintMock: vi.fn(),
}))

vi.mock('@fohte/service-kit/observability', () => ({
  captureWithFingerprint: captureWithFingerprintMock,
}))

afterEach(() => vi.restoreAllMocks())

const MODEL_NAME = 'example-model-envelope-test'

const buildToolCallResponse = (
  callId: string,
  name: string,
  args: Record<string, unknown>,
): Response =>
  new Response(
    JSON.stringify({
      id: callId,
      model: MODEL_NAME,
      choices: [
        {
          index: 0,
          finish_reason: 'tool_calls',
          message: {
            role: 'assistant',
            content: null,
            tool_calls: [
              {
                id: callId,
                type: 'function',
                function: { name, arguments: JSON.stringify(args) },
              },
            ],
          },
        },
      ],
    }),
    { status: 200, headers: { 'content-type': 'application/json' } },
  )

const buildTextResponse = (callId: string): Response =>
  new Response(
    JSON.stringify({
      id: callId,
      model: MODEL_NAME,
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

const buildMcpTool = (
  func: (args: {
    readonly label: string
    readonly explicitNull: string | null
    readonly omitted?: string
    readonly values: (string | number | boolean | null)[]
    readonly amount: number
    readonly enabled: boolean
    readonly nested: { readonly note: string | null }
  }) => Promise<[string, unknown]>,
): DynamicStructuredTool =>
  new DynamicStructuredTool({
    name: 'demo_lookup',
    description: 'Returns a demo result.',
    schema: z.object({
      label: z.string(),
      explicitNull: z.string().nullable(),
      omitted: z.string().optional(),
      values: z.array(z.union([z.string(), z.number(), z.boolean(), z.null()])),
      amount: z.number(),
      enabled: z.boolean(),
      nested: z.object({ note: z.string().nullable() }),
    }),
    responseFormat: 'content_and_artifact',
    func,
  })

const buildAgent = (
  mcpTool: DynamicStructuredTool,
  fetch: ChatOpenAIFetch,
  localTool?: DynamicStructuredTool,
) =>
  createAgent({
    model: createStubChatModel(fetch, { model: MODEL_NAME }),
    tools: [mcpTool, ...(localTool !== undefined ? [localTool] : [])],
    middleware: [
      createStrategyToolErrorMiddleware(),
      createMcpToolJsonEnvelopeMiddleware([mcpTool]),
    ],
  })

const startToolCallThenFinish =
  (argsJson: string, requests: string[]): ChatOpenAIFetch =>
  (_url, init) => {
    const body = init?.body
    if (typeof body !== 'string') throw new Error('expected request body')
    requests.push(body)
    return Promise.resolve(
      requests.length === 1
        ? buildToolCallResponse('call-1', 'demo_lookup', {
            args_json: argsJson,
          })
        : buildTextResponse(`response-${String(requests.length)}`),
    )
  }

const normalizeToolMessages = (messages: readonly BaseMessage[]) =>
  messages
    .filter((message) => ToolMessage.isInstance(message))
    .map((message) => ({
      name: message.name,
      tool_call_id: message.tool_call_id,
      status: message.status,
      content: message.status === 'error' ? '<tool error>' : message.text,
      artifact: message.artifact as unknown,
    }))

const normalizeSuccessfulRun = (
  modelTools: unknown[],
  receivedArgs: unknown[],
  toolMessage: ToolMessage | undefined,
  finalMessage: string | undefined,
) => ({
  modelTools,
  receivedArgs,
  toolMessage:
    toolMessage === undefined
      ? undefined
      : {
          name: toolMessage.name,
          tool_call_id: toolMessage.tool_call_id,
          status: toolMessage.status,
          content: toolMessage.text,
          artifact: toolMessage.artifact as unknown,
        },
  finalMessage,
})

const normalizeErrorRun = (
  modelCallCount: number,
  receivedArgs: unknown[],
  messages: readonly BaseMessage[],
  finalMessage: string | undefined,
) => ({
  modelCallCount,
  receivedArgs,
  toolMessages: normalizeToolMessages(messages),
  finalMessage,
})

describe('createMcpToolJsonEnvelopeMiddleware', () => {
  it('preserves MCP arguments and artifact output while leaving local tool schemas unchanged', async () => {
    const receivedArgs: unknown[] = []
    const mcpTool = buildMcpTool((args) => {
      receivedArgs.push(args)
      return Promise.resolve(['lookup completed', { marker: 'retained' }])
    })
    const localTool = new DynamicStructuredTool({
      name: 'local_action',
      description: 'Runs a local action.',
      schema: z.object({ query: z.string() }),
      func: () => Promise.resolve('unused'),
    })
    const requests: string[] = []
    const inputArgs = {
      label: 'sample value',
      explicitNull: null,
      values: ['item', 7, true, null],
      amount: 12,
      enabled: false,
      nested: { note: 'nested value' },
    }
    const agent = buildAgent(
      mcpTool,
      startToolCallThenFinish(JSON.stringify(inputArgs), requests),
      localTool,
    )
    const result = await agent.invoke({
      messages: [new HumanMessage('continue')],
    })
    const request = z
      .object({ tools: z.array(z.unknown()) })
      .parse(JSON.parse(requests[0] ?? '{}'))
    const toolMessage = result.messages.find((message) =>
      ToolMessage.isInstance(message),
    )
    const originalMcpSchema = convertToOpenAITool(mcpTool).function.parameters
    const expectedMcpTool = {
      type: 'function',
      function: {
        name: mcpTool.name,
        description: [
          mcpTool.description,
          'Pass the complete tool arguments as a JSON object string in args_json.',
          `Original input JSON Schema: ${JSON.stringify(originalMcpSchema)}`,
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

    expect(
      normalizeSuccessfulRun(
        request.tools,
        receivedArgs,
        toolMessage,
        result.messages.at(-1)?.text,
      ),
    ).toEqual({
      modelTools: [expectedMcpTool, convertToOpenAITool(localTool)],
      receivedArgs: [inputArgs],
      toolMessage: {
        name: 'demo_lookup',
        tool_call_id: 'call-1',
        status: 'success',
        content: 'lookup completed',
        artifact: { marker: 'retained' },
      },
      finalMessage: 'done',
    })
  })

  it.each([
    { name: 'invalid JSON', argsJson: '{' },
    { name: 'schema-invalid object', argsJson: '{"label":7}' },
  ])(
    'returns an error without calling the MCP tool for $name',
    async ({ argsJson }) => {
      const receivedArgs: unknown[] = []
      const mcpTool = buildMcpTool((args) => {
        receivedArgs.push(args)
        return Promise.resolve(['lookup completed', { marker: 'retained' }])
      })
      const requests: string[] = []
      const agent = buildAgent(
        mcpTool,
        startToolCallThenFinish(argsJson, requests),
      )
      const result = await agent.invoke({
        messages: [new HumanMessage('continue')],
      })

      expect(
        normalizeErrorRun(
          requests.length,
          receivedArgs,
          result.messages,
          result.messages.at(-1)?.text,
        ),
      ).toEqual({
        modelCallCount: 2,
        receivedArgs: [],
        toolMessages: [
          {
            name: 'demo_lookup',
            tool_call_id: 'call-1',
            status: 'error',
            content: '<tool error>',
            artifact: undefined,
          },
        ],
        finalMessage: 'done',
      })
    },
  )
})
