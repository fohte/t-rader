import type { Message } from '@a2a-js/sdk'
import { BaseChatModel } from '@langchain/core/language_models/chat_models'
import { HumanMessage } from '@langchain/core/messages'
import type { ChatResult } from '@langchain/core/outputs'
import { DynamicStructuredTool } from '@langchain/core/tools'
import { ChatOpenAI } from '@langchain/openai'
import { errAsync, okAsync } from 'neverthrow'
import { describe, expect, it, vi } from 'vitest'
import { z } from 'zod'

import type { AgentConfig } from '#strategy-agent/agent-config-client'
import { AgentConfigFetchError } from '#strategy-agent/agent-config-client'
import type { CompiledPhaseAgent } from '#strategy-agent/agent-graph/run-agent-graph'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import { MAX_MODEL_CALLS_PER_INVOKE } from '#strategy-agent/final-turn-middleware'
import type {
  CompiledStrategyAgent,
  McpToolsClient,
  StrategyAgentDeps,
} from '#strategy-agent/strategy-agent'
import {
  createStrategyAgentDeps,
  runStrategyAgent,
} from '#strategy-agent/strategy-agent'

class FakeChatModel extends BaseChatModel {
  override _llmType(): string {
    return 'fake'
  }

  override _generate(): Promise<ChatResult> {
    return Promise.reject(
      new Error('FakeChatModel should never be invoked directly in tests'),
    )
  }
}

const buildFakeTool = (name: string): DynamicStructuredTool =>
  new DynamicStructuredTool({
    name,
    description: `fake ${name} tool`,
    schema: z.object({}),
    func: () => Promise.resolve('unused in these tests'),
  })

const buildUserMessage = (text: string): Message => ({
  kind: 'message',
  role: 'user',
  messageId: 'm1',
  parts: [{ kind: 'text', text }],
})

// NoopTracer (テスト環境では実 exporter を設定しないため) が返す固定の invalid
// span context。@opentelemetry/api の INVALID_TRACEID/INVALID_SPANID と同じ値。
const NOOP_TRACE_ID = '00000000000000000000000000000000'
const NOOP_SPAN_ID = '0000000000000000'

// startedAt/finishedAt は実行のたびに変わるため、比較前に固定文字列へ正規化する。
const normalizeStepTimestamps = (
  steps: readonly StrategyTaskStep[],
): unknown[] =>
  steps.map((step) => ({
    ...step,
    startedAt: '<started-at>',
    ...(step.finishedAt !== undefined ? { finishedAt: '<finished-at>' } : {}),
  }))

const AGENT_CONFIG: AgentConfig = {
  agentsMd: '# AGENTS',
  skills: { 'ja-stock': 'skill body' },
  model: 'opencode-go/minimax-m3',
  smallModel: 'opencode-go/deepseek-v4-flash',
  agentGraph: '',
}

interface BuildDepsOptions {
  readonly agentInvoke: CompiledStrategyAgent['invoke']
  readonly tools?: readonly DynamicStructuredTool[]
  readonly agentGraph?: string
  readonly buildPhaseAgentInvoke?: CompiledPhaseAgent['invoke']
}

interface Calls {
  fetchAgentConfigStrategyId?: string
  createMcpClientStrategyId?: string
  mcpClientClosed: boolean
  createChatModelArg?: string
  createChatModelReturnValue?: unknown
  buildAgentOptions?: {
    model: unknown
    tools: readonly DynamicStructuredTool[]
    systemPrompt: string
  }
}

const buildDeps = (
  options: BuildDepsOptions,
): { deps: StrategyAgentDeps; calls: Calls } => {
  const calls: Calls = { mcpClientClosed: false }
  const chatModel = new FakeChatModel({})

  const deps: StrategyAgentDeps = {
    fetchAgentConfig: (strategyId) => {
      calls.fetchAgentConfigStrategyId = strategyId
      return okAsync({
        ...AGENT_CONFIG,
        agentGraph: options.agentGraph ?? AGENT_CONFIG.agentGraph,
      })
    },
    createMcpClient: (strategyId): McpToolsClient => {
      calls.createMcpClientStrategyId = strategyId
      return {
        getTools: () => Promise.resolve([...(options.tools ?? [])]),
        close: () => {
          calls.mcpClientClosed = true
          return Promise.resolve()
        },
      }
    },
    createChatModel: (model) => {
      calls.createChatModelArg = model
      calls.createChatModelReturnValue = chatModel
      return chatModel
    },
    buildAgent: (buildOptions) => {
      calls.buildAgentOptions = buildOptions
      return {
        invoke: (input) => options.agentInvoke(input),
      }
    },
    buildPhaseAgent: () => ({
      invoke: (input) =>
        (
          options.buildPhaseAgentInvoke ??
          (() => Promise.resolve({ structuredResponse: {} }))
        )(input),
    }),
  }

  return { deps, calls }
}

describe('runStrategyAgent', () => {
  it('fetches the agent config, builds the agent with it, and maps a completed structured response', async () => {
    const mcpTools = [buildFakeTool('query_data'), buildFakeTool('write_note')]
    const { deps, calls } = buildDeps({
      tools: mcpTools,
      agentInvoke: () =>
        Promise.resolve({
          structuredResponse: { status: 'completed', message: 'done' },
        }),
    })

    const result = await runStrategyAgent(
      deps,
      'strategy-1',
      buildUserMessage('do the thing'),
    )

    expect.soft(result).toEqual({ status: 'completed', message: 'done' })
    expect.soft(calls.fetchAgentConfigStrategyId).toBe('strategy-1')
    expect.soft(calls.createMcpClientStrategyId).toBe('strategy-1')
    expect.soft(calls.createChatModelArg).toBe('opencode-go/minimax-m3')
    expect
      .soft(calls.buildAgentOptions?.systemPrompt)
      .toBe('# AGENTS\n\n# Skill: ja-stock\n\nskill body')
    expect.soft(calls.buildAgentOptions?.tools).toEqual(mcpTools)
    expect
      .soft(calls.buildAgentOptions?.model)
      .toBe(calls.createChatModelReturnValue)
    expect.soft(calls.mcpClientClosed).toBe(true)
  })

  it('maps an "error" structured response to failed with error_kind agent_error', async () => {
    const { deps } = buildDeps({
      agentInvoke: () =>
        Promise.resolve({
          structuredResponse: { status: 'error', message: 'could not comply' },
        }),
    })

    const result = await runStrategyAgent(
      deps,
      'strategy-1',
      buildUserMessage('do the thing'),
    )

    expect(result).toEqual({
      status: 'failed',
      message: 'could not comply',
      errorKind: 'agent_error',
    })
  })

  it('maps a missing structured response to failed with error_kind agent_error', async () => {
    const { deps } = buildDeps({
      agentInvoke: () => Promise.resolve({}),
    })

    const result = await runStrategyAgent(
      deps,
      'strategy-1',
      buildUserMessage('do the thing'),
    )

    expect(result).toEqual({
      status: 'failed',
      message: 'agent did not return a structured response',
      errorKind: 'agent_error',
    })
  })

  it('maps a thrown usage-limit error to error_kind usage_limit and still closes the MCP client', async () => {
    const { deps, calls } = buildDeps({
      agentInvoke: () =>
        Promise.reject(
          Object.assign(new Error('rate limited'), {
            rateLimitType: 'capacity',
          }),
        ),
    })

    const result = await runStrategyAgent(
      deps,
      'strategy-1',
      buildUserMessage('do the thing'),
    )

    expect(result).toEqual({
      status: 'failed',
      message: 'usage limit reached',
      errorKind: 'usage_limit',
    })
    expect(calls.mcpClientClosed).toBe(true)
  })

  it('maps a generic thrown error to error_kind agent_error using its message', async () => {
    const { deps } = buildDeps({
      agentInvoke: () => Promise.reject(new Error('mcp tool blew up')),
    })

    const result = await runStrategyAgent(
      deps,
      'strategy-1',
      buildUserMessage('do the thing'),
    )

    expect(result).toEqual({
      status: 'failed',
      message: 'mcp tool blew up',
      errorKind: 'agent_error',
    })
  })

  it('maps a fetchAgentConfig error result to error_kind agent_error and still closes the MCP client', async () => {
    const { deps, calls } = buildDeps({
      agentInvoke: () => Promise.reject(new Error('should not be invoked')),
    })
    const fetchError = new AgentConfigFetchError(
      'failed to fetch agent config for strategy strategy-1: 500',
    )

    const result = await runStrategyAgent(
      { ...deps, fetchAgentConfig: () => errAsync(fetchError) },
      'strategy-1',
      buildUserMessage('do the thing'),
    )

    expect(result).toEqual({
      status: 'failed',
      message: fetchError.message,
      errorKind: 'agent_error',
    })
    expect(calls.mcpClientClosed).toBe(true)
  })

  it('delegates to runAgentGraph when agent_graph is configured', async () => {
    const { deps } = buildDeps({
      agentGraph:
        'phases:\n  - key: p\n    label: P\n    model: m\n    prompt: do p\n',
      agentInvoke: () =>
        Promise.reject(new Error('buildAgent should not be invoked')),
    })

    const result = await runStrategyAgent(
      deps,
      'strategy-1',
      buildUserMessage('do the thing'),
    )

    expect(result).toEqual({
      status: 'completed',
      message: '1フェーズの実行が完了しました (P)',
    })
  })

  it('forwards onStepsChanged through to runAgentGraph when agent_graph is configured', async () => {
    const { deps } = buildDeps({
      agentGraph:
        'phases:\n  - key: p\n    label: P\n    model: m\n    prompt: do p\n',
      agentInvoke: () =>
        Promise.reject(new Error('buildAgent should not be invoked')),
    })
    const notifications: (readonly StrategyTaskStep[])[] = []

    const result = await runStrategyAgent(
      deps,
      'strategy-1',
      buildUserMessage('do the thing'),
      (steps) => notifications.push(steps),
    )

    expect(result).toEqual({
      status: 'completed',
      message: '1フェーズの実行が完了しました (P)',
    })
    expect(notifications.map(normalizeStepTimestamps)).toEqual([
      [
        {
          phaseKey: 'p',
          label: 'P',
          model: 'm',
          status: 'running',
          startedAt: '<started-at>',
          traceId: NOOP_TRACE_ID,
          spanId: NOOP_SPAN_ID,
        },
      ],
      [
        {
          phaseKey: 'p',
          label: 'P',
          model: 'm',
          status: 'completed',
          output: {},
          startedAt: '<started-at>',
          finishedAt: '<finished-at>',
          traceId: NOOP_TRACE_ID,
          spanId: NOOP_SPAN_ID,
        },
      ],
    ])
  })

  it('fails fast on malformed agent_graph without invoking any agent', async () => {
    const { deps } = buildDeps({
      agentGraph: 'phases: [',
      agentInvoke: () => Promise.reject(new Error('should not be invoked')),
      buildPhaseAgentInvoke: () =>
        Promise.reject(new Error('should not be invoked')),
    })

    const result = await runStrategyAgent(
      deps,
      'strategy-1',
      buildUserMessage('do the thing'),
    )

    expect(result).toEqual({
      status: 'failed',
      message: 'agent_graph is not valid YAML',
      errorKind: 'agent_error',
    })
  })
})

describe('createStrategyAgentDeps', () => {
  const baseConfig = {
    backendApiBaseUrl: 'http://t-rader-backend',
    strategyMcpUrl: 'http://t-rader-backend/mcp/strategy',
    llmApiKey: 'test-key',
    genAiProviderName: 'opencode',
  }

  const expectChatOpenAI = (model: BaseChatModel) => {
    if (!(model instanceof ChatOpenAI)) throw new Error('expected ChatOpenAI')
    return model
  }

  it('creates a chat model defaulted to the OpenCode Go base URL', () => {
    const deps = createStrategyAgentDeps(baseConfig)

    const model = expectChatOpenAI(deps.createChatModel('test-model'))

    expect(model.clientConfig.baseURL).toBe('https://opencode.ai/zen/go/v1')
  })

  it('accepts a base URL override', () => {
    const deps = createStrategyAgentDeps({
      ...baseConfig,
      llmBaseUrl: 'https://litellm.example.com/v1',
    })

    const model = expectChatOpenAI(deps.createChatModel('test-model'))

    expect(model.clientConfig.baseURL).toBe('https://litellm.example.com/v1')
  })

  it('omits reasoning when no reasoning effort is given', () => {
    const deps = createStrategyAgentDeps(baseConfig)

    const model = expectChatOpenAI(deps.createChatModel('test-model'))

    expect(model.reasoning).toBeUndefined()
  })

  it('passes the reasoning effort through to the chat model', () => {
    const deps = createStrategyAgentDeps(baseConfig)

    const model = expectChatOpenAI(
      deps.createChatModel('test-model', { reasoningEffort: 'high' }),
    )

    expect(model.reasoning).toEqual({ effort: 'high' })
  })

  type ChatOpenAIFetch = NonNullable<
    NonNullable<ConstructorParameters<typeof ChatOpenAI>[0]>['configuration']
  >['fetch']

  const buildStubModel = (fetch: ChatOpenAIFetch): ChatOpenAI =>
    new ChatOpenAI({
      apiKey: 'test-key',
      model: 'chatgpt/gpt-5',
      maxRetries: 0,
      configuration: { baseURL: 'http://localhost', fetch },
    })

  const chatCompletionsRequestSchema = z.object({
    messages: z.array(z.object({ role: z.string(), content: z.unknown() })),
  })

  it('sends the system prompt as string content, not an array, over the wire', async () => {
    let requestBody: z.infer<typeof chatCompletionsRequestSchema> = {
      messages: [],
    }
    const model = buildStubModel((_url, init) => {
      const body = init?.body
      if (typeof body !== 'string') throw new Error('expected string body')
      requestBody = chatCompletionsRequestSchema.parse(JSON.parse(body))
      return Promise.resolve(new Response('', { status: 500 }))
    })
    const deps = createStrategyAgentDeps(baseConfig)

    const agent = deps.buildAgent({
      model,
      tools: [],
      systemPrompt: 'you are a helpful bot',
    })
    // 実プロダクトでは 200 を返すが、ここでは request body を捕捉した時点で
    // 目的を達成しているため、この後の失敗レスポンス処理は捨ててよい。
    await agent
      .invoke({ messages: [new HumanMessage('hi')] })
      .catch(() => undefined)

    const systemMessage = requestBody.messages.find(
      (message) => message.role === 'system',
    )
    expect(systemMessage?.content).toBe('you are a helpful bot')
  })

  const toolCallRequestSchema = z.object({
    tools: z.array(z.object({ function: z.object({ name: z.string() }) })),
  })

  // OpenAI chat completions のレスポンス envelope は全呼び出しで不変。
  // このテストで実際に効くのは toolCall (どの tool を呼ぶか) だけ。
  const buildToolCallResponse = (
    callId: string,
    toolCall: { name: string | undefined; arguments: string },
  ): Response =>
    new Response(
      JSON.stringify({
        id: callId,
        model: 'chatgpt/gpt-5',
        choices: [
          {
            index: 0,
            finish_reason: 'tool_calls',
            message: {
              role: 'assistant',
              content: null,
              tool_calls: [
                { id: callId, type: 'function', function: toolCall },
              ],
            },
          },
        ],
      }),
      { status: 200, headers: { 'content-type': 'application/json' } },
    )

  it('drops regular tools once MAX_MODEL_CALLS_PER_INVOKE is reached, forcing the structured-output tool', async () => {
    const requestedToolCounts: number[] = []
    let callCount = 0
    const model = buildStubModel((_url, init) => {
      callCount += 1
      const body = init?.body
      if (typeof body !== 'string') throw new Error('expected string body')
      const { tools } = toolCallRequestSchema.parse(JSON.parse(body))
      requestedToolCounts.push(tools.length)
      // 通常 tool が外された最終ターンでは提出用 tool だけが残る。それ以外の
      // ターンでは常に search tool を呼び続け、自発的な提出をさせない。
      const isFinalTurn = tools.length === 1
      const toolCall = isFinalTurn
        ? {
            name: tools[0]?.function.name,
            arguments: JSON.stringify({
              status: 'completed',
              message: 'done',
            }),
          }
        : { name: 'search', arguments: '{}' }
      return Promise.resolve(
        buildToolCallResponse(`call-${String(callCount)}`, toolCall),
      )
    })
    const deps = createStrategyAgentDeps(baseConfig)

    const agent = deps.buildAgent({
      model,
      tools: [buildFakeTool('search')],
      systemPrompt: 'you are a helpful bot',
    })
    const result = await agent.invoke({ messages: [new HumanMessage('hi')] })

    expect(result.structuredResponse).toEqual({
      status: 'completed',
      message: 'done',
    })
    expect(requestedToolCounts).toEqual([
      ...Array<number>(MAX_MODEL_CALLS_PER_INVOKE - 1).fill(2),
      1,
    ])
  })

  it('logs immediately when the model ends without a structured-output tool call', async () => {
    // モデルが構造化出力 tool を一切呼ばずプレーンな文章で終える応答。
    const model = buildStubModel(() =>
      Promise.resolve(
        new Response(
          JSON.stringify({
            id: 'call-1',
            model: 'chatgpt/gpt-5',
            choices: [
              {
                index: 0,
                finish_reason: 'stop',
                message: { role: 'assistant', content: 'no tool for you' },
              },
            ],
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        ),
      ),
    )
    const deps = createStrategyAgentDeps(baseConfig)

    const agent = deps.buildAgent({
      model,
      tools: [buildFakeTool('search')],
      systemPrompt: 'you are a helpful bot',
    })

    const warnSpy = vi
      .spyOn(console, 'warn')
      .mockImplementation(() => undefined)
    try {
      const result = await agent.invoke({ messages: [new HumanMessage('hi')] })

      expect(result).toEqual({})
      expect(warnSpy.mock.calls).toEqual([
        [
          'modelResponseGuardMiddleware: model call ended without a structured-output tool call',
        ],
      ])
    } finally {
      warnSpy.mockRestore()
    }
  })

  it('logs an undeclared tool call immediately, then lets the model recover and submit structured output', async () => {
    let callCount = 0
    const model = buildStubModel((_url, init) => {
      callCount += 1
      const body = init?.body
      if (typeof body !== 'string') throw new Error('expected string body')
      const { tools } = toolCallRequestSchema.parse(JSON.parse(body))
      // 1 回目: 宣言されていない tool を呼ぶ。ToolNode がこれを自動で
      // エラーの ToolMessage に変換し、2 回目の呼び出しへつながる。
      if (callCount === 1) {
        return Promise.resolve(
          buildToolCallResponse('call-1', {
            name: 'ghost_tool',
            arguments: '{}',
          }),
        )
      }
      // 2 回目: 宣言済みの `search` 以外の tool (=構造化出力 tool、動的な
      // `extract-N` 名) を呼んで正常終了させる。
      const structuredOutputToolName = tools.find(
        (tool) => tool.function.name !== 'search',
      )?.function.name
      return Promise.resolve(
        buildToolCallResponse('call-2', {
          name: structuredOutputToolName,
          arguments: JSON.stringify({
            status: 'completed',
            message: 'done',
          }),
        }),
      )
    })
    const deps = createStrategyAgentDeps(baseConfig)

    const agent = deps.buildAgent({
      model,
      tools: [buildFakeTool('search')],
      systemPrompt: 'you are a helpful bot',
    })

    const warnSpy = vi
      .spyOn(console, 'warn')
      .mockImplementation(() => undefined)
    try {
      const result = await agent.invoke({ messages: [new HumanMessage('hi')] })

      expect(result.structuredResponse).toEqual({
        status: 'completed',
        message: 'done',
      })
      expect(warnSpy.mock.calls).toEqual([
        [
          'modelResponseGuardMiddleware: model called undeclared tool(s): ghost_tool',
        ],
      ])
    } finally {
      warnSpy.mockRestore()
    }
  })
})
