import type { Message } from '@a2a-js/sdk'
import { BaseChatModel } from '@langchain/core/language_models/chat_models'
import { HumanMessage } from '@langchain/core/messages'
import type { ChatResult } from '@langchain/core/outputs'
import { DynamicStructuredTool } from '@langchain/core/tools'
import { ChatOpenAI } from '@langchain/openai'
import { errAsync, okAsync } from 'neverthrow'
import { describe, expect, it, vi } from 'vitest'
import { z } from 'zod'

import { logger } from '#logger'
import type {
  AgentConfig,
  FetchAgentConfig,
} from '#strategy-agent/agent-config-client'
import type {
  BuildPhaseAgentOptions,
  CompiledPhaseAgent,
} from '#strategy-agent/agent-graph/run-agent-graph'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import { MAX_MODEL_CALLS_PER_INVOKE } from '#strategy-agent/final-turn-middleware'
import type {
  McpToolsClient,
  RunStrategyAgentInput,
  StrategyAgentDeps,
} from '#strategy-agent/strategy-agent'
import {
  createStrategyAgentDeps,
  runStrategyAgent,
} from '#strategy-agent/strategy-agent'
import { MAX_TOOL_CALLS_PER_MODEL_CALL } from '#strategy-agent/tool-call-cap-middleware'
import { createFirstOccurrenceLabeler } from '#test/first-occurrence-labeler'
import { normalizeStepTimestamps } from '#test/normalize-step-timestamps'

let capturedMcpClientConfig: unknown

vi.mock('@langchain/mcp-adapters', () => ({
  MultiServerMCPClient: vi.fn(function (config: unknown) {
    capturedMcpClientConfig = config
  }),
}))

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

const buildRunInput = (
  overrides: Partial<RunStrategyAgentInput> = {},
): RunStrategyAgentInput => ({
  strategyId: 'strategy-1',
  purpose: undefined,
  taskId: 'task-1',
  userMessage: buildUserMessage('do the thing'),
  resumeSteps: undefined,
  asOf: undefined,
  deadlineSignal: undefined,
  ...overrides,
})

// NoopTracer (テスト環境では実 exporter を設定しないため) が返す固定の invalid
// span context。@opentelemetry/api の INVALID_TRACEID/INVALID_SPANID と同じ値。
const NOOP_TRACE_ID = '00000000000000000000000000000000'
const NOOP_SPAN_ID = '0000000000000000'

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/

const AGENT_GRAPH = `phases:
  - key: work
    label: Work
    model: example-model-work
    prompt: Complete the task
`

const AGENT_CONFIG: AgentConfig = {
  agentsMd: '# AGENTS',
  skills: { 'ja-stock': 'skill body' },
  agentGraph: AGENT_GRAPH,
}

interface BuildDepsOptions {
  readonly tools?: readonly DynamicStructuredTool[]
  readonly agentGraph?: string
  readonly buildPhaseAgentInvoke?: (
    input: Parameters<CompiledPhaseAgent['invoke']>[0],
    calls: Calls,
  ) => ReturnType<CompiledPhaseAgent['invoke']>
}

interface McpClientCall {
  readonly executionId: string
  closed: boolean
}

interface Calls {
  fetchAgentConfigKey?: Parameters<FetchAgentConfig>[0]
  mcpClientClosed: boolean
  // createMcpClient の呼び出しごとの 1 行。生成数と close タイミングの検証用。
  mcpClients: McpClientCall[]
  mcpClientToolModels: Readonly<Record<string, string>>[]
  capturedDeadlineSignal: AbortSignal | undefined
}

const buildDeps = (
  options: BuildDepsOptions,
): { deps: StrategyAgentDeps; calls: Calls } => {
  const calls: Calls = {
    mcpClientClosed: false,
    mcpClients: [],
    mcpClientToolModels: [],
    capturedDeadlineSignal: undefined,
  }
  const chatModel = new FakeChatModel({})

  const deps: StrategyAgentDeps = {
    fetchAgentConfig: (key) => {
      calls.fetchAgentConfigKey = key
      return okAsync({
        ...AGENT_CONFIG,
        agentGraph: options.agentGraph ?? AGENT_CONFIG.agentGraph,
      })
    },
    createMcpClient: (_strategyId, executionId, toolModels): McpToolsClient => {
      calls.mcpClientToolModels.push(toolModels)
      const client: McpClientCall = { executionId, closed: false }
      calls.mcpClients.push(client)
      return {
        getTools: () => Promise.resolve([...(options.tools ?? [])]),
        close: () => {
          calls.mcpClientClosed = true
          client.closed = true
          return Promise.resolve()
        },
      }
    },
    createChatModel: () => chatModel,
    buildPhaseAgent: (buildOptions) => {
      calls.capturedDeadlineSignal = buildOptions.deadlineSignal
      return {
        invoke: (input) =>
          options.buildPhaseAgentInvoke !== undefined
            ? options.buildPhaseAgentInvoke(input, calls)
            : Promise.resolve({ structuredResponse: {} }),
      }
    },
  }

  return { deps, calls }
}

const buildPhaseAgentUnderTest = (
  deps: StrategyAgentDeps,
  options: Omit<BuildPhaseAgentOptions, 'responseSchema'>,
): CompiledPhaseAgent =>
  deps.buildPhaseAgent({
    ...options,
    responseSchema: {
      type: 'object',
      properties: {
        status: { type: 'string' },
        message: { type: 'string' },
      },
      required: ['status', 'message'],
    },
  })

type PhaseAgentResult = Awaited<ReturnType<CompiledPhaseAgent['invoke']>>

const normalizeSuccessfulRetry = (
  result: PhaseAgentResult,
  requestBodies: readonly string[],
) => ({
  structuredResponse: result.structuredResponse,
  requestBodies,
})

const normalizeSuccessfulRetryWithWarnings = (
  result: PhaseAgentResult,
  requestBodies: readonly string[],
  retryWarnings: readonly {
    readonly name: string
    readonly message: string
  }[],
) => ({
  ...normalizeSuccessfulRetry(result, requestBodies),
  retryWarnings,
})

const normalizeAttemptOutcome = (callCount: number, outcome: string) => ({
  callCount,
  outcome,
})

const normalizeTimedRetry = (
  result: PhaseAgentResult,
  requestBodies: readonly string[],
  signals: readonly (AbortSignal | undefined)[],
) => ({
  ...normalizeSuccessfulRetry(result, requestBodies),
  signalsAborted: signals.map((signal) => signal?.aborted ?? false),
  signalsAreDistinct: signals[0] !== signals[1],
})

const normalizeDeadlineOutcome = <T>(
  callCount: number,
  outcome: string,
  signal: AbortSignal | undefined,
  warnCalls: T,
) => ({
  callCount,
  outcome,
  signalAborted: signal?.aborted ?? false,
  warnCalls,
})

describe('runStrategyAgent', () => {
  it('passes the as_of time to the phase agent when agent_graph is configured', async () => {
    let invokedMessages: unknown
    const { deps } = buildDeps({
      agentGraph:
        'phases:\n  - key: p\n    label: P\n    model: m\n    prompt: do p\n',
      buildPhaseAgentInvoke: (input) => {
        invokedMessages = input.messages
        return Promise.resolve({ structuredResponse: {} })
      },
    })

    await runStrategyAgent(
      deps,
      buildRunInput({ asOf: new Date('2026-01-02T03:04:05Z') }),
    )

    expect(invokedMessages).toEqual([
      new HumanMessage(
        [
          [
            '基準時刻 (as_of): 2026-01-02T03:04:05.000Z',
            'これは実行の論理的な基準時刻であり、参照するデータがすべてこの時刻のものであることは保証されない。',
            'do the thing',
          ].join('\n\n'),
          'do p',
        ].join('\n\n---\n\n'),
      ),
    ])
  })

  it('fetches the agent config and runs the configured graph', async () => {
    const mcpTools = [buildFakeTool('query_data'), buildFakeTool('write_note')]
    const { deps } = buildDeps({ tools: mcpTools })

    const result = await runStrategyAgent(deps, buildRunInput())

    expect(result).toEqual({
      status: 'completed',
      message: '1フェーズの実行が完了しました (Work)',
    })
  })

  it.each([
    {
      name: 'configured tool_models',
      agentGraph: `tool_models:\n  search_web: example-model-search\n  query_media: example-model-media\n${AGENT_GRAPH}`,
      expectedToolModels: {
        search_web: 'example-model-search',
        query_media: 'example-model-media',
      },
    },
    {
      name: 'omitted tool_models',
      agentGraph: AGENT_GRAPH,
      expectedToolModels: {},
    },
  ])(
    'passes $name to the MCP client',
    async ({ agentGraph, expectedToolModels }) => {
      const { deps, calls } = buildDeps({ agentGraph })

      await runStrategyAgent(deps, buildRunInput())

      expect(calls.mcpClientToolModels).toEqual([expectedToolModels])
    },
  )

  it('passes the given purpose straight through to fetchAgentConfig instead of the default', async () => {
    const { deps, calls } = buildDeps({})

    await runStrategyAgent(deps, buildRunInput({ purpose: 'purpose-a' }))

    expect(calls.fetchAgentConfigKey).toEqual({
      purpose: 'purpose-a',
    })
  })

  it('fails when agent_graph is not configured', async () => {
    const { deps } = buildDeps({ agentGraph: '' })

    const result = await runStrategyAgent(deps, buildRunInput())

    expect(result).toEqual({
      status: 'failed',
      message: 'agent_graph is not configured',
      errorKind: 'agent_error',
    })
  })

  it('maps a thrown usage-limit error from a phase to error_kind usage_limit', async () => {
    const { deps } = buildDeps({
      buildPhaseAgentInvoke: () =>
        Promise.reject(
          Object.assign(new Error('rate limited'), {
            rateLimitType: 'capacity',
          }),
        ),
    })

    const result = await runStrategyAgent(deps, buildRunInput())

    expect(result).toEqual({
      status: 'failed',
      message: 'フェーズ「Work」(work) の実行に失敗しました: rate limited',
      errorKind: 'usage_limit',
    })
  })

  it('maps a generic thrown phase error to error_kind agent_error', async () => {
    const { deps } = buildDeps({
      buildPhaseAgentInvoke: () =>
        Promise.reject(new Error('mcp tool blew up')),
    })

    const result = await runStrategyAgent(deps, buildRunInput())

    expect(result).toEqual({
      status: 'failed',
      message: 'フェーズ「Work」(work) の実行に失敗しました: mcp tool blew up',
      errorKind: 'agent_error',
    })
  })

  it('maps a fetchAgentConfig error', async () => {
    const { deps } = buildDeps({})
    const fetchError = new Error(
      'failed to fetch agent config for strategy strategy-1: 500',
    )

    const result = await runStrategyAgent(
      { ...deps, fetchAgentConfig: () => errAsync(fetchError) },
      buildRunInput(),
    )

    expect(result).toEqual({
      status: 'failed',
      message: fetchError.message,
      errorKind: 'agent_error',
    })
  })

  it('delegates to runAgentGraph when agent_graph is configured', async () => {
    const { deps } = buildDeps({
      agentGraph:
        'phases:\n  - key: p\n    label: P\n    model: m\n    prompt: do p\n',
    })

    const result = await runStrategyAgent(deps, buildRunInput())

    expect(result).toEqual({
      status: 'completed',
      message: '1フェーズの実行が完了しました (P)',
    })
  })

  it('forwards onStepsChanged through to runAgentGraph when agent_graph is configured', async () => {
    const { deps } = buildDeps({
      agentGraph:
        'phases:\n  - key: p\n    label: P\n    model: m\n    prompt: do p\n',
    })
    const notifications: (readonly StrategyTaskStep[])[] = []

    const result = await runStrategyAgent(
      deps,
      buildRunInput({ onStepsChanged: (steps) => notifications.push(steps) }),
    )

    expect(result).toEqual({
      status: 'completed',
      message: '1フェーズの実行が完了しました (P)',
    })
    expect(notifications.map(normalizeStepTimestamps)).toEqual([
      [
        {
          phaseKey: 'p',
          executionStepId: '<execution-step-id-1>',
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
          executionStepId: '<execution-step-id-1>',
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

  it('forwards deadlineSignal through to runAgentGraph (and then buildPhaseAgent) when agent_graph is configured', async () => {
    const controller = new AbortController()
    const { deps, calls } = buildDeps({
      agentGraph:
        'phases:\n  - key: p\n    label: P\n    model: m\n    prompt: do p\n',
    })

    const result = await runStrategyAgent(
      deps,
      buildRunInput({ deadlineSignal: controller.signal }),
    )

    expect(result).toEqual({
      status: 'completed',
      message: '1フェーズの実行が完了しました (P)',
    })
    expect(calls.capturedDeadlineSignal).toBe(controller.signal)
  })

  it('skips a completed phase on resume when a valid resume step is provided', async () => {
    let buildPhaseAgentInvokeCalls = 0
    const { deps } = buildDeps({
      agentGraph:
        'phases:\n  - key: p\n    label: P\n    model: m\n    prompt: do p\n',
      buildPhaseAgentInvoke: () => {
        buildPhaseAgentInvokeCalls++
        return Promise.resolve({ structuredResponse: {} })
      },
    })
    const resumeSteps: unknown[] = [
      {
        phase_key: 'p',
        execution_step_id: '11111111-1111-1111-1111-111111111111',
        label: 'P',
        model: 'm',
        status: 'completed',
        output: { note: 'from-resume' },
        started_at: '2020-01-01T00:00:00.000Z',
        finished_at: '2020-01-01T00:00:01.000Z',
        trace_id: 'trace-1',
        span_id: 'span-1',
      },
    ]

    const result = await runStrategyAgent(deps, buildRunInput({ resumeSteps }))

    expect(result).toEqual({
      status: 'completed',
      message: '1フェーズの実行が完了しました (P)',
    })
    expect(buildPhaseAgentInvokeCalls).toBe(0)
  })

  it('ignores resume steps that fail schema validation and runs the phase fresh', async () => {
    let buildPhaseAgentInvokeCalls = 0
    const { deps } = buildDeps({
      agentGraph:
        'phases:\n  - key: p\n    label: P\n    model: m\n    prompt: do p\n',
      buildPhaseAgentInvoke: () => {
        buildPhaseAgentInvokeCalls++
        return Promise.resolve({ structuredResponse: {} })
      },
    })
    // phase_key など必須フィールドを欠いており strategyTaskStepJsonSchema を通らない。
    const resumeSteps: unknown[] = [{ status: 'completed' }]

    const result = await runStrategyAgent(deps, buildRunInput({ resumeSteps }))

    expect(result).toEqual({
      status: 'completed',
      message: '1フェーズの実行が完了しました (P)',
    })
    expect(buildPhaseAgentInvokeCalls).toBe(1)
  })

  it('opens one MCP client per graph step and closes it before the next step starts', async () => {
    const snapshots: McpClientCall[][] = []
    const { deps, calls } = buildDeps({
      agentGraph: [
        'phases:',
        '  - key: plan',
        '    label: Plan',
        '    model: m',
        '    prompt: do plan',
        '  - key: work',
        '    label: Work',
        '    model: m',
        '    prompt: do work',
        '    for_each: plan.items',
        '    max_parallel: 1',
        '',
      ].join('\n'),
      buildPhaseAgentInvoke: (_input, currentCalls) => {
        snapshots.push(currentCalls.mcpClients.map((c) => ({ ...c })))
        return Promise.resolve({ structuredResponse: { items: ['a', 'b'] } })
      },
    })

    const result = await runStrategyAgent(deps, buildRunInput())

    // ステップ ID は crypto.randomUUID() 由来のため、出現順に番号を振って比較する。
    const label = createFirstOccurrenceLabeler('step')
    const normalize = (clients: readonly McpClientCall[]): McpClientCall[] =>
      clients.map((client) => {
        const [prefix, stepId] = client.executionId.split(':')
        expect(stepId).toMatch(UUID_PATTERN)
        return {
          executionId: `${String(prefix)}:${label(String(stepId))}`,
          closed: client.closed,
        }
      })

    expect.soft(result).toEqual({
      status: 'completed',
      message: '2フェーズの実行が完了しました (Plan → Work)',
    })
    expect.soft(snapshots.map(normalize)).toEqual([
      [{ executionId: 'task-1:<step-1>', closed: false }],
      [
        { executionId: 'task-1:<step-1>', closed: true },
        { executionId: 'task-1:<step-2>', closed: false },
      ],
      [
        { executionId: 'task-1:<step-1>', closed: true },
        { executionId: 'task-1:<step-2>', closed: true },
        { executionId: 'task-1:<step-3>', closed: false },
      ],
    ])
    expect.soft(normalize(calls.mcpClients)).toEqual([
      { executionId: 'task-1:<step-1>', closed: true },
      { executionId: 'task-1:<step-2>', closed: true },
      { executionId: 'task-1:<step-3>', closed: true },
    ])
  })

  it('fails fast on malformed agent_graph without invoking any agent', async () => {
    const { deps } = buildDeps({
      agentGraph: 'phases: [',
      buildPhaseAgentInvoke: () =>
        Promise.reject(new Error('should not be invoked')),
    })

    const result = await runStrategyAgent(deps, buildRunInput())

    expect(result).toEqual({
      status: 'failed',
      message: 'agent_graph is not valid YAML',
      errorKind: 'agent_error',
    })
  })
})

describe('createStrategyAgentDeps', () => {
  const baseConfig = {
    backendApiBaseUrl: 'http://t-rader-backend/',
    llmApiKey: 'test-key',
    genAiProviderName: 'opencode',
    llmCallTimeoutMs: 600_000,
  }

  const expectChatOpenAI = (model: BaseChatModel) => {
    if (!(model instanceof ChatOpenAI)) throw new Error('expected ChatOpenAI')
    return model
  }

  it('bakes the strategy and execution ids into the MCP client headers at construction', () => {
    createStrategyAgentDeps(baseConfig).createMcpClient(
      'strategy-1',
      'task-1:step-1',
      { search_web: 'example-model-search' },
    )

    expect(capturedMcpClientConfig).toEqual({
      mcpServers: {
        strategy: {
          url: 'http://t-rader-backend/mcp/strategy',
          headers: {
            'x-strategy-id': 'strategy-1',
            'x-execution-id': 'task-1:step-1',
            'x-tool-models': '{"search_web":"example-model-search"}',
          },
        },
      },
    })
  })

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

  it('configures the chat model with the configured call timeout', () => {
    const deps = createStrategyAgentDeps({
      ...baseConfig,
      llmCallTimeoutMs: 123_000,
    })

    const model = expectChatOpenAI(deps.createChatModel('test-model'))

    expect(model.timeout).toBe(123_000)
  })

  it('disables retries so a single call is bounded by the configured timeout', () => {
    const deps = createStrategyAgentDeps(baseConfig)

    const model = expectChatOpenAI(deps.createChatModel('test-model'))

    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- caller.maxRetries は @langchain/core の型定義上 protected だが、ChatOpenAI に渡した maxRetries が実際に反映される唯一の観測点のため構造的に narrowing する。
    const caller = model.caller as unknown as { maxRetries: number }
    expect(caller.maxRetries).toBe(0)
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

  const buildStubModel = (
    fetch: ChatOpenAIFetch,
    streaming = false,
  ): ChatOpenAI =>
    new ChatOpenAI({
      apiKey: 'test-key',
      model: 'example-model-test-stream',
      maxRetries: 0,
      streaming,
      configuration: { baseURL: 'http://localhost', fetch },
    })

  type ChatOpenAIRequestInit = Parameters<NonNullable<ChatOpenAIFetch>>[1]

  const getRequestBody = (init: ChatOpenAIRequestInit): string => {
    const body = init?.body
    if (typeof body !== 'string') throw new Error('expected string body')
    return body
  }

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

    const agent = buildPhaseAgentUnderTest(deps, {
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
        model: 'example-model-test-stream',
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

  // 宣言済み tool から structured-output 用の動的な tool 名 (`extract-N` 等) を
  // 拾う。test 対象の agent は必ずこれを宣言するため、無ければテスト側のバグ。
  const pickStructuredOutputToolName = (
    tools: z.infer<typeof toolCallRequestSchema>['tools'],
  ): string => {
    const name = tools.find((tool) => tool.function.name !== 'search')?.function
      .name
    if (name === undefined) {
      throw new Error('expected a structured-output tool to be declared')
    }
    return name
  }

  const buildStructuredOutputResponse = (
    callId: string,
    requestBody: string,
  ): Response => {
    const { tools } = toolCallRequestSchema.parse(JSON.parse(requestBody))
    return buildToolCallResponse(callId, {
      name: pickStructuredOutputToolName(tools),
      arguments: JSON.stringify({ status: 'completed', message: 'done' }),
    })
  }

  const buildStructuredOutputStreamResponse = (
    callId: string,
    requestBody: string,
  ): Response => {
    const { tools } = toolCallRequestSchema.parse(JSON.parse(requestBody))
    const encoder = new TextEncoder()
    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(
          encoder.encode(
            `data: ${JSON.stringify({
              id: callId,
              model: 'example-model-test-stream',
              choices: [
                {
                  index: 0,
                  finish_reason: null,
                  delta: {
                    role: 'assistant',
                    tool_calls: [
                      {
                        index: 0,
                        id: callId,
                        type: 'function',
                        function: {
                          name: pickStructuredOutputToolName(tools),
                          arguments: JSON.stringify({
                            status: 'completed',
                            message: 'done',
                          }),
                        },
                      },
                    ],
                  },
                },
              ],
            })}\n\n`,
          ),
        )
        controller.enqueue(
          encoder.encode(
            `data: ${JSON.stringify({
              id: callId,
              model: 'example-model-test-stream',
              choices: [{ index: 0, finish_reason: 'tool_calls', delta: {} }],
            })}\n\n`,
          ),
        )
        controller.enqueue(encoder.encode('data: [DONE]\n\n'))
        controller.close()
      },
    })
    return new Response(stream, {
      status: 200,
      headers: { 'content-type': 'text/event-stream' },
    })
  }

  const buildPartialModelStreamResponse = (
    ending:
      | { readonly kind: 'sse-error' }
      | { readonly kind: 'read-error'; readonly message: string },
  ): Response => {
    const encoder = new TextEncoder()
    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(
          encoder.encode(
            `data: ${JSON.stringify({
              id: 'call-1',
              model: 'example-model-test-stream',
              choices: [
                {
                  index: 0,
                  finish_reason: null,
                  delta: { role: 'assistant', content: 'partial' },
                },
              ],
            })}\n\n`,
          ),
        )
        if (ending.kind === 'sse-error') {
          controller.enqueue(
            encoder.encode(
              `data: ${JSON.stringify({
                error: {
                  message: 'stream interrupted',
                  type: 'server_error',
                },
              })}\n\n`,
            ),
          )
          controller.close()
          return
        }
        controller.error(new TypeError(ending.message))
      },
    })
    return new Response(stream, {
      status: 200,
      headers: { 'content-type': 'text/event-stream' },
    })
  }

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

    const agent = buildPhaseAgentUnderTest(deps, {
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
            model: 'example-model-test-stream',
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

    const agent = buildPhaseAgentUnderTest(deps, {
      model,
      tools: [buildFakeTool('search')],
      systemPrompt: 'you are a helpful bot',
    })

    const warnSpy = vi.spyOn(logger, 'warn').mockImplementation(() => undefined)
    try {
      const result = await agent.invoke({ messages: [new HumanMessage('hi')] })

      expect(result).toEqual({})
      expect(warnSpy.mock.calls).toEqual([
        [
          {},
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
      return Promise.resolve(
        buildToolCallResponse('call-2', {
          name: pickStructuredOutputToolName(tools),
          arguments: JSON.stringify({
            status: 'completed',
            message: 'done',
          }),
        }),
      )
    })
    const deps = createStrategyAgentDeps(baseConfig)

    const agent = buildPhaseAgentUnderTest(deps, {
      model,
      tools: [buildFakeTool('search')],
      systemPrompt: 'you are a helpful bot',
    })

    const warnSpy = vi.spyOn(logger, 'warn').mockImplementation(() => undefined)
    try {
      const result = await agent.invoke({ messages: [new HumanMessage('hi')] })

      expect(result.structuredResponse).toEqual({
        status: 'completed',
        message: 'done',
      })
      expect(warnSpy.mock.calls).toEqual([
        [
          { undeclaredNames: ['ghost_tool'] },
          'modelResponseGuardMiddleware: model called undeclared tool(s): ghost_tool',
        ],
      ])
    } finally {
      warnSpy.mockRestore()
    }
  })

  it.each([
    {
      name: 'tool result messages',
      role: 'tool',
      expected: {
        role: 'tool',
        content: 'unused in these tests',
        tool_call_id: 'call-1',
      },
    },
    {
      name: 'the assistant message replayed on the next model call',
      role: 'assistant',
      expected: {
        role: 'assistant',
        content: '',
        tool_calls: [
          {
            id: 'call-1',
            type: 'function',
            function: { name: 'search', arguments: '{}' },
          },
        ],
      },
    },
  ])(
    'omits name from $name, since some upstream providers reject it',
    async ({ role, expected }) => {
      let callCount = 0
      let capturedMessage: unknown
      const model = buildStubModel((_url, init) => {
        callCount += 1
        const body = init?.body
        if (typeof body !== 'string') throw new Error('expected string body')
        const { messages, tools } = z
          .object({
            messages: z.array(z.record(z.string(), z.unknown())),
            tools: toolCallRequestSchema.shape.tools,
          })
          .parse(JSON.parse(body))
        if (callCount === 1) {
          return Promise.resolve(
            buildToolCallResponse('call-1', {
              name: 'search',
              arguments: '{}',
            }),
          )
        }
        capturedMessage = messages.find((message) => message['role'] === role)
        return Promise.resolve(
          buildToolCallResponse('call-2', {
            name: pickStructuredOutputToolName(tools),
            arguments: JSON.stringify({ status: 'completed', message: 'done' }),
          }),
        )
      })
      const deps = createStrategyAgentDeps(baseConfig)

      const agent = buildPhaseAgentUnderTest(deps, {
        model,
        tools: [buildFakeTool('search')],
        systemPrompt: 'you are a helpful bot',
      })
      const result = await agent.invoke({ messages: [new HumanMessage('hi')] })

      expect(result.structuredResponse).toEqual({
        status: 'completed',
        message: 'done',
      })
      expect(capturedMessage).toEqual(expected)
    },
  )

  it.each([
    {
      name: 'a 5xx response',
      streaming: false,
      failureResponse: () =>
        new Response(
          JSON.stringify({
            error: { message: 'temporary failure', type: 'server_error' },
          }),
          {
            status: 503,
            headers: { 'content-type': 'application/json' },
          },
        ),
    },
    {
      name: 'a statusless SSE error event',
      streaming: true,
      failureResponse: () =>
        buildPartialModelStreamResponse({ kind: 'sse-error' }),
    },
    {
      name: 'a stream connection closing',
      streaming: true,
      failureResponse: () =>
        buildPartialModelStreamResponse({
          kind: 'read-error',
          message: 'terminated',
        }),
    },
  ])(
    'retries after $name with the same input',
    async ({ failureResponse, streaming }) => {
      const requestBodies: string[] = []
      let callCount = 0
      const model = buildStubModel((_url, init) => {
        callCount += 1
        const body = getRequestBody(init)
        requestBodies.push(body)
        if (callCount === 1) return Promise.resolve(failureResponse())
        const response = streaming
          ? buildStructuredOutputStreamResponse(
              `call-${String(callCount)}`,
              body,
            )
          : buildStructuredOutputResponse(`call-${String(callCount)}`, body)
        return Promise.resolve(response)
      }, streaming)
      const deps = createStrategyAgentDeps(baseConfig)
      const agent = buildPhaseAgentUnderTest(deps, {
        model,
        tools: [],
        systemPrompt: 'you are a helpful bot',
      })

      const result = await agent.invoke({ messages: [new HumanMessage('hi')] })
      const firstRequestBody = requestBodies[0]

      expect(normalizeSuccessfulRetry(result, requestBodies)).toEqual({
        structuredResponse: { status: 'completed', message: 'done' },
        requestBodies: [firstRequestBody, firstRequestBody],
      })
    },
  )

  it('does not retry after a non-transport TypeError while reading a stream', async () => {
    let callCount = 0
    const model = buildStubModel(() => {
      callCount += 1
      return Promise.resolve(
        buildPartialModelStreamResponse({
          kind: 'read-error',
          message: 'invalid model stream data',
        }),
      )
    }, true)
    const deps = createStrategyAgentDeps(baseConfig)
    const agent = buildPhaseAgentUnderTest(deps, {
      model,
      tools: [],
      systemPrompt: 'you are a helpful bot',
    })
    const outcome = await agent
      .invoke({ messages: [new HumanMessage('hi')] })
      .then(
        () => 'resolved',
        () => 'rejected',
      )

    expect(normalizeAttemptOutcome(callCount, outcome)).toEqual({
      callCount: 1,
      outcome: 'rejected',
    })
  })

  it.each([
    {
      name: 'a fetch connection error',
      makeError: () => new TypeError('connection closed'),
      expectedLoggedError: {
        name: 'TypeError',
        message: 'connection closed',
      },
    },
    {
      name: 'a request timeout',
      makeError: () => new DOMException('request timed out', 'TimeoutError'),
      expectedLoggedError: {
        name: 'TimeoutError',
        message: 'Request timed out.',
      },
    },
  ])(
    'retries after $name and logs its cause',
    async ({ makeError, expectedLoggedError }) => {
      const requestBodies: string[] = []
      let callCount = 0
      const cause = makeError()
      const model = buildStubModel((_url, init) => {
        callCount += 1
        const body = getRequestBody(init)
        requestBodies.push(body)
        if (callCount === 1) return Promise.reject(cause)
        return Promise.resolve(
          buildStructuredOutputResponse(`call-${String(callCount)}`, body),
        )
      })
      const deps = createStrategyAgentDeps(baseConfig)
      const agent = buildPhaseAgentUnderTest(deps, {
        model,
        tools: [],
        systemPrompt: 'you are a helpful bot',
      })
      const warnSpy = vi
        .spyOn(logger, 'warn')
        .mockImplementation(() => undefined)

      try {
        const result = await agent.invoke({
          messages: [new HumanMessage('hi')],
        })
        const firstRequestBody = requestBodies[0]
        const retryWarnings = warnSpy.mock.calls
          .filter(
            ([, message]) =>
              message === 'model call failed with a retryable error',
          )
          .map(([fields]) => {
            const error = fields['err']
            return error instanceof Error
              ? { name: error.name, message: error.message }
              : { name: 'missing error', message: 'missing error' }
          })

        expect(
          normalizeSuccessfulRetryWithWarnings(
            result,
            requestBodies,
            retryWarnings,
          ),
        ).toEqual({
          structuredResponse: { status: 'completed', message: 'done' },
          requestBodies: [firstRequestBody, firstRequestBody],
          retryWarnings: [expectedLoggedError],
        })
      } finally {
        warnSpy.mockRestore()
      }
    },
  )

  it('stops after two retries when each model call returns a 5xx response', async () => {
    let callCount = 0
    const model = buildStubModel(() => {
      callCount += 1
      return Promise.resolve(
        new Response(
          JSON.stringify({
            error: { message: 'temporary failure', type: 'server_error' },
          }),
          { status: 503, headers: { 'content-type': 'application/json' } },
        ),
      )
    })
    const deps = createStrategyAgentDeps(baseConfig)
    const agent = buildPhaseAgentUnderTest(deps, {
      model,
      tools: [],
      systemPrompt: 'you are a helpful bot',
    })
    const outcome = await agent
      .invoke({ messages: [new HumanMessage('hi')] })
      .then(
        () => 'resolved',
        () => 'rejected',
      )

    expect(normalizeAttemptOutcome(callCount, outcome)).toEqual({
      callCount: 3,
      outcome: 'rejected',
    })
  })

  it.each([{ status: 400 }, { status: 401 }, { status: 404 }, { status: 429 }])(
    'does not retry after an HTTP $status response',
    async ({ status }) => {
      let callCount = 0
      const model = buildStubModel(() => {
        callCount += 1
        return Promise.resolve(
          new Response(
            JSON.stringify({
              error: {
                message: 'request rejected',
                type: 'invalid_request_error',
              },
            }),
            {
              status,
              headers: { 'content-type': 'application/json' },
            },
          ),
        )
      })
      const deps = createStrategyAgentDeps(baseConfig)
      const agent = buildPhaseAgentUnderTest(deps, {
        model,
        tools: [],
        systemPrompt: 'you are a helpful bot',
      })
      const outcome = await agent
        .invoke({ messages: [new HumanMessage('hi')] })
        .then(
          () => 'resolved',
          () => 'rejected',
        )

      expect(normalizeAttemptOutcome(callCount, outcome)).toEqual({
        callCount: 1,
        outcome: 'rejected',
      })
    },
  )

  it('retries with a fresh signal after llmCallTimeoutMs elapses mid-stream', async () => {
    const capturedSignals: Array<AbortSignal | undefined> = []
    const requestBodies: string[] = []
    let callCount = 0
    const model = buildStubModel((_url, init) => {
      callCount += 1
      const body = getRequestBody(init)
      requestBodies.push(body)
      capturedSignals.push(init?.signal ?? undefined)
      if (callCount > 1) {
        return Promise.resolve(
          buildStructuredOutputResponse(`call-${String(callCount)}`, body),
        )
      }
      // ストリームが流れ続けて resolve/reject しない応答を模す。signal が
      // 実際に fetch まで届いて abort されない限り、この Promise は解決しない。
      return new Promise((_resolve, reject) => {
        init?.signal?.addEventListener('abort', () => {
          reject(new Error('aborted'))
        })
      })
    })
    const deps = createStrategyAgentDeps({
      ...baseConfig,
      llmCallTimeoutMs: 10,
    })

    const agent = buildPhaseAgentUnderTest(deps, {
      model,
      tools: [],
      systemPrompt: 'you are a helpful bot',
    })

    const result = await agent.invoke({ messages: [new HumanMessage('hi')] })
    const firstRequestBody = requestBodies[0]

    expect(normalizeTimedRetry(result, requestBodies, capturedSignals)).toEqual(
      {
        structuredResponse: { status: 'completed', message: 'done' },
        requestBodies: [firstRequestBody, firstRequestBody],
        signalsAborted: [true, false],
        signalsAreDistinct: true,
      },
    )
  })

  it('aborts the underlying HTTP request when deadlineSignal is aborted mid-stream', async () => {
    const controller = new AbortController()
    let capturedSignal: AbortSignal | undefined
    let callCount = 0
    const model = buildStubModel((_url, init) => {
      callCount += 1
      capturedSignal = init?.signal ?? undefined
      // 実リクエストが飛んだ後に deadline 超過を模して abort する。
      setTimeout(() => {
        controller.abort()
      }, 10)
      // ストリームが流れ続けて resolve/reject しない応答を模す。signal が
      // 実際に fetch まで届いて abort されない限り、この Promise は解決しない。
      return new Promise((_resolve, reject) => {
        init?.signal?.addEventListener('abort', () => {
          reject(new Error('aborted'))
        })
      })
    })
    const deps = createStrategyAgentDeps(baseConfig)

    const agent = buildPhaseAgentUnderTest(deps, {
      model,
      tools: [],
      systemPrompt: 'you are a helpful bot',
      deadlineSignal: controller.signal,
    })

    const warnSpy = vi.spyOn(logger, 'warn').mockImplementation(() => undefined)
    try {
      const outcome = await agent
        .invoke({ messages: [new HumanMessage('hi')] })
        .then(
          () => 'resolved',
          (error: unknown) =>
            error instanceof Error
              ? `${error.name}: ${error.message}`
              : String(error),
        )

      expect(
        normalizeDeadlineOutcome(
          callCount,
          outcome,
          capturedSignal,
          warnSpy.mock.calls,
        ),
      ).toEqual({
        callCount: 1,
        outcome:
          'Error: deadlineMiddleware: aborted model call after strategy task deadline exceeded',
        signalAborted: true,
        warnCalls: [
          [
            {},
            'deadlineMiddleware: aborted model call after strategy task deadline exceeded',
          ],
        ],
      })
    } finally {
      warnSpy.mockRestore()
    }
  })

  it('aborts the model call once distinct tool call indices exceed MAX_TOOL_CALLS_PER_MODEL_CALL, even mid-stream', async () => {
    const encoder = new TextEncoder()
    // OpenAI chat completions のストリーミング delta。1 chunk につき
    // 新しい index の tool_call_chunk を 1 件ずつ流す。
    const toolCallDeltaChunk = (index: number): Uint8Array =>
      encoder.encode(
        `data: ${JSON.stringify({
          id: 'call-1',
          model: 'example-model-test-stream',
          choices: [
            {
              index: 0,
              finish_reason: null,
              delta: {
                role: 'assistant',
                tool_calls: [
                  {
                    index,
                    id: `call-${String(index)}`,
                    type: 'function',
                    function: { name: 'search', arguments: '{}' },
                  },
                ],
              },
            },
          ],
        })}\n\n`,
      )

    const model = new ChatOpenAI({
      apiKey: 'test-key',
      model: 'example-model-test-stream',
      maxRetries: 0,
      streaming: true,
      configuration: {
        baseURL: 'http://localhost',
        fetch: (_url, init) => {
          const signal = init?.signal
          const stream = new ReadableStream<Uint8Array>({
            async start(controller) {
              const onAbort = (): void => {
                controller.error(new Error('tool call cap test: aborted'))
              }
              signal?.addEventListener('abort', onAbort)
              // 打ち切りしきい値を大きく超える件数を用意し、最後まで
              // 送り切る前に実際に打ち切られることを検証する。
              const totalToolCalls = MAX_TOOL_CALLS_PER_MODEL_CALL * 4
              for (let index = 0; index < totalToolCalls; index += 1) {
                if (signal?.aborted === true) return
                controller.enqueue(toolCallDeltaChunk(index))
                // handleLLMNewToken の非同期キューを消化させるため、タイマー境界を挟む。
                await new Promise((resolve) => setTimeout(resolve, 0))
              }
              signal?.removeEventListener('abort', onAbort)
              controller.enqueue(encoder.encode('data: [DONE]\n\n'))
              controller.close()
            },
          })
          return Promise.resolve(
            new Response(stream, {
              status: 200,
              headers: { 'content-type': 'text/event-stream' },
            }),
          )
        },
      },
    })
    const deps = createStrategyAgentDeps(baseConfig)

    const agent = buildPhaseAgentUnderTest(deps, {
      model,
      tools: [buildFakeTool('search')],
      systemPrompt: 'you are a helpful bot',
    })

    const warnSpy = vi.spyOn(logger, 'warn').mockImplementation(() => undefined)
    try {
      await expect(
        agent.invoke({ messages: [new HumanMessage('hi')] }),
      ).rejects.toThrow()

      expect(warnSpy.mock.calls).toEqual([
        [
          {
            toolCallCountsByName: { search: MAX_TOOL_CALLS_PER_MODEL_CALL + 1 },
          },
          `toolCallCapMiddleware: aborted model call after exceeding ${String(MAX_TOOL_CALLS_PER_MODEL_CALL)} tool call(s) in a single response (search: ${String(MAX_TOOL_CALLS_PER_MODEL_CALL + 1)})`,
        ],
      ])
    } finally {
      warnSpy.mockRestore()
    }
  })

  it('breaks down the abort message by tool name when multiple tools are called', async () => {
    const encoder = new TextEncoder()
    const toolNames = ['search', 'notes'] as const
    // OpenAI chat completions のストリーミング delta。1 chunk につき
    // 新しい index の tool_call_chunk を 1 件ずつ、tool 名を交互に流す。
    const toolCallDeltaChunk = (index: number): Uint8Array =>
      encoder.encode(
        `data: ${JSON.stringify({
          id: 'call-1',
          model: 'example-model-test-stream',
          choices: [
            {
              index: 0,
              finish_reason: null,
              delta: {
                role: 'assistant',
                tool_calls: [
                  {
                    index,
                    id: `call-${String(index)}`,
                    type: 'function',
                    function: {
                      name: toolNames[index % toolNames.length],
                      arguments: '{}',
                    },
                  },
                ],
              },
            },
          ],
        })}\n\n`,
      )

    const model = new ChatOpenAI({
      apiKey: 'test-key',
      model: 'example-model-test-stream',
      maxRetries: 0,
      streaming: true,
      configuration: {
        baseURL: 'http://localhost',
        fetch: (_url, init) => {
          const signal = init?.signal
          const stream = new ReadableStream<Uint8Array>({
            async start(controller) {
              const onAbort = (): void => {
                controller.error(new Error('tool call cap test: aborted'))
              }
              signal?.addEventListener('abort', onAbort)
              const totalToolCalls = MAX_TOOL_CALLS_PER_MODEL_CALL * 4
              for (let index = 0; index < totalToolCalls; index += 1) {
                if (signal?.aborted === true) return
                controller.enqueue(toolCallDeltaChunk(index))
                // handleLLMNewToken の非同期キューを消化させるため、タイマー境界を挟む。
                await new Promise((resolve) => setTimeout(resolve, 0))
              }
              signal?.removeEventListener('abort', onAbort)
              controller.enqueue(encoder.encode('data: [DONE]\n\n'))
              controller.close()
            },
          })
          return Promise.resolve(
            new Response(stream, {
              status: 200,
              headers: { 'content-type': 'text/event-stream' },
            }),
          )
        },
      },
    })
    const deps = createStrategyAgentDeps(baseConfig)

    const agent = buildPhaseAgentUnderTest(deps, {
      model,
      tools: [buildFakeTool('search'), buildFakeTool('notes')],
      systemPrompt: 'you are a helpful bot',
    })

    const warnSpy = vi.spyOn(logger, 'warn').mockImplementation(() => undefined)
    try {
      await expect(
        agent.invoke({ messages: [new HumanMessage('hi')] }),
      ).rejects.toThrow()

      // index 0..50 (計 51 件) を search/notes 交互に割り当てるため
      // search が 26 件、notes が 25 件になり、件数降順で並ぶ。
      expect(warnSpy.mock.calls).toEqual([
        [
          { toolCallCountsByName: { search: 26, notes: 25 } },
          `toolCallCapMiddleware: aborted model call after exceeding ${String(MAX_TOOL_CALLS_PER_MODEL_CALL)} tool call(s) in a single response (search: 26, notes: 25)`,
        ],
      ])
    } finally {
      warnSpy.mockRestore()
    }
  })

  it('completes normally when a response has exactly MAX_TOOL_CALLS_PER_MODEL_CALL distinct tool call indices', async () => {
    const encoder = new TextEncoder()
    const buildToolCallStream = (
      toolCalls: ReadonlyArray<{
        index: number
        id: string
        name: string
        arguments: string
      }>,
    ): ReadableStream<Uint8Array> =>
      new ReadableStream({
        async start(controller) {
          for (const toolCall of toolCalls) {
            controller.enqueue(
              encoder.encode(
                `data: ${JSON.stringify({
                  id: 'call-1',
                  model: 'example-model-test-stream',
                  choices: [
                    {
                      index: 0,
                      finish_reason: null,
                      delta: {
                        role: 'assistant',
                        tool_calls: [
                          {
                            index: toolCall.index,
                            id: toolCall.id,
                            type: 'function',
                            function: {
                              name: toolCall.name,
                              arguments: toolCall.arguments,
                            },
                          },
                        ],
                      },
                    },
                  ],
                })}\n\n`,
              ),
            )
            // handleLLMNewToken の非同期キューを消化させるため、タイマー境界を挟む。
            await new Promise((resolve) => setTimeout(resolve, 0))
          }
          controller.enqueue(
            encoder.encode(
              `data: ${JSON.stringify({
                id: 'call-1',
                model: 'example-model-test-stream',
                choices: [{ index: 0, finish_reason: 'tool_calls', delta: {} }],
              })}\n\n`,
            ),
          )
          controller.enqueue(encoder.encode('data: [DONE]\n\n'))
          controller.close()
        },
      })

    let callCount = 0
    const model = new ChatOpenAI({
      apiKey: 'test-key',
      model: 'example-model-test-stream',
      maxRetries: 0,
      streaming: true,
      configuration: {
        baseURL: 'http://localhost',
        fetch: (_url, init) => {
          callCount += 1
          const body = init?.body
          if (typeof body !== 'string') throw new Error('expected string body')

          if (callCount === 1) {
            const toolCalls = Array.from(
              { length: MAX_TOOL_CALLS_PER_MODEL_CALL },
              (_, index) => ({
                index,
                id: `call-${String(index)}`,
                name: 'search',
                arguments: '{}',
              }),
            )
            return Promise.resolve(
              new Response(buildToolCallStream(toolCalls), {
                status: 200,
                headers: { 'content-type': 'text/event-stream' },
              }),
            )
          }

          const { tools } = toolCallRequestSchema.parse(JSON.parse(body))
          return Promise.resolve(
            new Response(
              buildToolCallStream([
                {
                  index: 0,
                  id: 'call-final',
                  name: pickStructuredOutputToolName(tools),
                  arguments: JSON.stringify({
                    status: 'completed',
                    message: 'done',
                  }),
                },
              ]),
              {
                status: 200,
                headers: { 'content-type': 'text/event-stream' },
              },
            ),
          )
        },
      },
    })
    const deps = createStrategyAgentDeps(baseConfig)

    const agent = buildPhaseAgentUnderTest(deps, {
      model,
      tools: [buildFakeTool('search')],
      systemPrompt: 'you are a helpful bot',
    })

    const warnSpy = vi.spyOn(logger, 'warn').mockImplementation(() => undefined)
    try {
      const result = await agent.invoke({ messages: [new HumanMessage('hi')] })

      expect(result.structuredResponse).toEqual({
        status: 'completed',
        message: 'done',
      })
      expect(warnSpy).not.toHaveBeenCalled()
    } finally {
      warnSpy.mockRestore()
    }
  })
})
