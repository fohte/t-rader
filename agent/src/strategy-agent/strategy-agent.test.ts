import type { Message } from '@a2a-js/sdk'
import { BaseCallbackHandler } from '@langchain/core/callbacks/base'
import { BaseChatModel } from '@langchain/core/language_models/chat_models'
import type { ChatResult } from '@langchain/core/outputs'
import { DynamicStructuredTool } from '@langchain/core/tools'
import { ChatOpenAI } from '@langchain/openai'
import { errAsync, okAsync } from 'neverthrow'
import { describe, expect, it } from 'vitest'
import { z } from 'zod'

import type { AgentConfig } from '#strategy-agent/agent-config-client'
import { AgentConfigFetchError } from '#strategy-agent/agent-config-client'
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

class FakeCallbackHandler extends BaseCallbackHandler {
  name = 'fake-callback-handler'
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

const AGENT_CONFIG: AgentConfig = {
  agentsMd: '# AGENTS',
  skills: { 'ja-stock': 'skill body' },
  model: 'opencode-go/minimax-m3',
  smallModel: 'opencode-go/deepseek-v4-flash',
}

interface BuildDepsOptions {
  readonly agentInvoke: CompiledStrategyAgent['invoke']
  readonly tools?: readonly DynamicStructuredTool[]
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
  invokeCallbacks?: readonly BaseCallbackHandler[]
}

const buildDeps = (
  options: BuildDepsOptions,
): { deps: StrategyAgentDeps; calls: Calls } => {
  const calls: Calls = { mcpClientClosed: false }
  const genAiCallbackHandler = new FakeCallbackHandler()
  const chatModel = new FakeChatModel({})

  const deps: StrategyAgentDeps = {
    fetchAgentConfig: (strategyId) => {
      calls.fetchAgentConfigStrategyId = strategyId
      return okAsync(AGENT_CONFIG)
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
        invoke: (input, callOptions) => {
          calls.invokeCallbacks = callOptions.callbacks
          return options.agentInvoke(input, callOptions)
        },
      }
    },
    genAiCallbackHandler,
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
    expect.soft(calls.invokeCallbacks).toEqual([deps.genAiCallbackHandler])
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
})

describe('createStrategyAgentDeps', () => {
  const baseConfig = {
    backendApiBaseUrl: 'http://t-rader-backend',
    strategyMcpUrl: 'http://t-rader-backend/mcp/strategy',
    openCodeApiKey: 'test-key',
    genAiCallbackHandler: new FakeCallbackHandler(),
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
})
