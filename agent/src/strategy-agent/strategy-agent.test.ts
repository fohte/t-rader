import type { Message } from '@a2a-js/sdk'
import { BaseChatModel } from '@langchain/core/language_models/chat_models'
import { HumanMessage } from '@langchain/core/messages'
import type { ChatResult } from '@langchain/core/outputs'
import { DynamicStructuredTool } from '@langchain/core/tools'
import { ChatOpenAI } from '@langchain/openai'
import { errAsync, okAsync } from 'neverthrow'
import { describe, expect, it } from 'vitest'
import { z } from 'zod'

import type { AgentConfig } from '#strategy-agent/agent-config-client'
import { AgentConfigFetchError } from '#strategy-agent/agent-config-client'
import type { CompiledPhaseAgent } from '#strategy-agent/agent-graph/run-agent-graph'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
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

  it('creates a chat model defaulted to the OpenCode Go base URL', () => {
    const deps = createStrategyAgentDeps(baseConfig)

    const model = deps.createChatModel('test-model')

    if (!(model instanceof ChatOpenAI)) throw new Error('expected ChatOpenAI')
    expect(model.clientConfig.baseURL).toBe('https://opencode.ai/zen/go/v1')
  })

  it('accepts a base URL override', () => {
    const deps = createStrategyAgentDeps({
      ...baseConfig,
      llmBaseUrl: 'https://litellm.example.com/v1',
    })

    const model = deps.createChatModel('test-model')

    if (!(model instanceof ChatOpenAI)) throw new Error('expected ChatOpenAI')
    expect(model.clientConfig.baseURL).toBe('https://litellm.example.com/v1')
  })

  const chatCompletionsRequestSchema = z.object({
    messages: z.array(z.object({ role: z.string(), content: z.unknown() })),
  })

  it('sends the system prompt as string content, not an array, over the wire', async () => {
    let requestBody: z.infer<typeof chatCompletionsRequestSchema> = {
      messages: [],
    }
    const model = new ChatOpenAI({
      apiKey: 'test-key',
      model: 'chatgpt/gpt-5',
      maxRetries: 0,
      configuration: {
        baseURL: 'http://localhost',
        fetch: (_url, init) => {
          const body = init?.body
          if (typeof body !== 'string') throw new Error('expected string body')
          requestBody = chatCompletionsRequestSchema.parse(JSON.parse(body))
          return Promise.resolve(new Response('', { status: 500 }))
        },
      },
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
})
