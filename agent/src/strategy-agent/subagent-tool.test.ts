import { BaseChatModel } from '@langchain/core/language_models/chat_models'
import { AIMessage, ToolMessage } from '@langchain/core/messages'
import type { ChatResult } from '@langchain/core/outputs'
import { DynamicStructuredTool } from '@langchain/core/tools'
import { errAsync, okAsync } from 'neverthrow'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { z } from 'zod'

import { createStepRecorder } from '#strategy-agent/agent-graph/run-agent-graph/step-execution'
import type {
  BuildPhaseAgentOptions,
  CompiledPhaseAgent,
  RunAgentGraphDeps,
} from '#strategy-agent/agent-graph/run-agent-graph/types'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import type { AgentGraphPhase } from '#strategy-agent/agent-graph/types'
import type { McpToolsClient } from '#strategy-agent/strategy-agent'
import type { SubagentDefinition } from '#strategy-agent/subagent-definition-client'
import { createRunSubagentTool } from '#strategy-agent/subagent-tool'
import { normalizeStepTimestamps } from '#test/normalize-step-timestamps'

class FakeChatModel extends BaseChatModel {
  override _llmType(): string {
    return 'fake'
  }

  override _generate(): Promise<ChatResult> {
    return Promise.reject(new Error('FakeChatModel should never be invoked'))
  }
}

const buildFakeTool = (name: string): DynamicStructuredTool =>
  new DynamicStructuredTool({
    name,
    description: `fake ${name} tool`,
    schema: z.object({}),
    func: () => Promise.resolve('unused in this test'),
  })

const phase: AgentGraphPhase = {
  key: 'research',
  label: 'Research',
  model: 'parent-model',
  prompt: 'Parent prompt',
  skills: [],
  subagents: ['researcher'],
  maxSubagentRuns: 2,
  output: {},
}

const definition: SubagentDefinition = {
  key: 'researcher',
  description: 'Investigate one question',
  model: 'sample-model',
  reasoningEffort: 'medium',
  prompt: 'Investigate the question and return evidence.',
  tools: ['search_web'],
  timeoutMinutes: 5,
  maxModelCalls: 3,
  toolLimits: { search_web: 4 },
  output: {
    required: ['conclusion'],
    conclusion: { type: 'string' },
    sources: { type: 'array', items: { type: 'string' } },
  },
}

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe('createRunSubagentTool', () => {
  it('runs only the requested definition in a separate conversation and records its step', async () => {
    vi.spyOn(crypto, 'randomUUID').mockReturnValue(
      'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
    )
    const buildOptions: BuildPhaseAgentOptions[] = []
    const childMessages: string[] = []
    const modelRequests: { model: string; reasoningEffort?: string }[] = []
    const createdMcpClients: {
      readonly executionStepId: string
      readonly definitionKey: string
    }[] = []
    const runAgentDeps: RunAgentGraphDeps = {
      createChatModel: (model, options) => {
        modelRequests.push({
          model,
          ...(options?.reasoningEffort !== undefined
            ? { reasoningEffort: options.reasoningEffort }
            : {}),
        })
        return new FakeChatModel({})
      },
      buildPhaseAgent: (options): CompiledPhaseAgent => {
        buildOptions.push(options)
        return {
          invoke: ({ messages }) => {
            childMessages.push(...messages.map((message) => message.text))
            return Promise.resolve({
              structuredResponse: {
                conclusion: 'A supported conclusion',
                sources: ['https://source.invalid/path'],
              },
            })
          },
        }
      },
    }
    const notifications: (readonly StrategyTaskStep[])[] = []
    const recorder = createStepRecorder((steps) => notifications.push(steps))
    const tool = createRunSubagentTool({
      phase,
      parentExecutionStepId: 'parent-step-id',
      recorder,
      fetchDefinition: () => okAsync(definition),
      createMcpClient: (executionStepId, definitionKey): McpToolsClient => {
        createdMcpClients.push({ executionStepId, definitionKey })
        return {
          getTools: () =>
            Promise.resolve([
              buildFakeTool('search_web'),
              buildFakeTool('read_page'),
              buildFakeTool('run_subagent'),
            ]),
          close: () => Promise.resolve(),
        }
      },
      deps: runAgentDeps,
    })

    const result = await tool.invoke({
      key: 'researcher',
      brief: 'Find evidence for the specified question.',
    })

    const actual = Array.of<unknown>(
      result,
      childMessages,
      modelRequests,
      buildOptions.map((options) => ({
        model: options.model instanceof FakeChatModel,
        tools: options.tools.map((toolItem) => toolItem.name),
        mcpTools: options.mcpTools?.map((toolItem) => toolItem.name),
        systemPrompt: options.systemPrompt,
        responseSchema: options.responseSchema,
        modelCallBudget: options.modelCallBudget,
        captureMessagesOnAbort: options.captureMessagesOnAbort,
      })),
      createdMcpClients,
      normalizeStepTimestamps(notifications.at(-1) ?? []),
    )

    expect(actual).toEqual([
      {
        conclusion: 'A supported conclusion',
        sources: ['https://source.invalid/path'],
      },
      ['Find evidence for the specified question.'],
      [{ model: 'sample-model', reasoningEffort: 'medium' }],
      [
        {
          model: true,
          tools: ['search_web'],
          mcpTools: ['search_web'],
          systemPrompt: 'Investigate the question and return evidence.',
          responseSchema: {
            type: 'object',
            properties: {
              conclusion: { type: 'string' },
              sources: { type: 'array', items: { type: 'string' } },
            },
            required: ['conclusion'],
            description:
              "Tool for extracting structured output from the model's response.",
          },
          modelCallBudget: { maxModelCalls: 3, count: 0 },
          captureMessagesOnAbort: true,
        },
      ],
      [
        {
          executionStepId: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
          definitionKey: 'researcher',
        },
      ],
      [
        {
          phaseKey: 'subagent:researcher',
          executionStepId: '<execution-step-id-1>',
          label: 'Investigate one question',
          model: 'sample-model',
          status: 'completed',
          item: { brief: 'Find evidence for the specified question.' },
          output: {
            conclusion: 'A supported conclusion',
            sources: ['https://source.invalid/path'],
          },
          startedAt: '<started-at>',
          finishedAt: '<finished-at>',
          traceId: '00000000000000000000000000000000',
          spanId: '0000000000000000',
        },
      ],
    ])
  })

  it('rejects a launch after max_subagent_runs is reached', async () => {
    const fetchDefinition = vi.fn(() => okAsync(definition))
    const tool = createRunSubagentTool({
      phase: { ...phase, maxSubagentRuns: 1 },
      parentExecutionStepId: 'parent-step-id',
      recorder: createStepRecorder(() => undefined),
      fetchDefinition,
      createMcpClient: () => ({
        getTools: () => Promise.resolve([]),
        close: () => Promise.resolve(),
      }),
      deps: {
        createChatModel: () => new FakeChatModel({}),
        buildPhaseAgent: () => ({
          invoke: () => Promise.resolve({ structuredResponse: {} }),
        }),
      },
    })
    const result = await tool.invoke({
      key: 'researcher',
      brief: 'Continue the investigation.',
    })
    const outcome = await tool
      .invoke({ key: 'researcher', brief: 'Continue the investigation.' })
      .then(
        (value) => ({ status: 'fulfilled' as const, value }),
        (error: unknown) => ({
          status: 'rejected' as const,
          message: error instanceof Error ? error.message : String(error),
        }),
      )

    const actual = Array.of<unknown>(
      result,
      outcome,
      fetchDefinition.mock.calls.length,
    )

    expect(actual).toEqual([
      {},
      {
        status: 'rejected',
        message: 'phase research exceeded max_subagent_runs 1',
      },
      1,
    ])
  })

  it('rejects a definition key that is not configured for the phase', async () => {
    const fetchDefinition = vi.fn(() => okAsync(definition))
    const tool = createRunSubagentTool({
      phase,
      parentExecutionStepId: 'parent-step-id',
      recorder: createStepRecorder(() => undefined),
      fetchDefinition,
      createMcpClient: () => ({
        getTools: () => Promise.resolve([]),
        close: () => Promise.resolve(),
      }),
      deps: {
        createChatModel: () => new FakeChatModel({}),
        buildPhaseAgent: () => ({
          invoke: () => Promise.resolve({ structuredResponse: {} }),
        }),
      },
    })
    const outcome = await tool
      .invoke({ key: 'outside', brief: 'Continue the investigation.' })
      .then(
        (value) => ({ status: 'fulfilled' as const, value }),
        (error: unknown) => ({
          status: 'rejected' as const,
          message: error instanceof Error ? error.message : String(error),
        }),
      )

    const actual = Array.of<unknown>(outcome, fetchDefinition.mock.calls.length)

    expect(actual).toEqual([
      {
        status: 'rejected',
        message: 'subagent key outside is not allowed in phase research',
      },
      0,
    ])
  })

  it('finalizes the child conversation when its timeout aborts the run', async () => {
    vi.useFakeTimers()
    const buildOptions: BuildPhaseAgentOptions[] = []
    const finalizerMessageHistory: unknown[][] = []
    const runAgentDeps: RunAgentGraphDeps = {
      createChatModel: () => new FakeChatModel({}),
      buildPhaseAgent: (options): CompiledPhaseAgent => {
        buildOptions.push(options)
        return {
          invoke: ({ messages }) => {
            if (options.tools.length === 0) {
              finalizerMessageHistory.push(
                messages.map((message) => {
                  if (message instanceof AIMessage) {
                    return {
                      role: 'assistant',
                      toolCalls: (message.tool_calls ?? []).map((toolCall) => ({
                        id: toolCall.id,
                        name: toolCall.name,
                        args: toolCall.args,
                      })),
                    }
                  }
                  if (message instanceof ToolMessage) {
                    return {
                      role: 'tool',
                      toolCallId: message.tool_call_id,
                      ...(message.status !== undefined
                        ? { status: message.status }
                        : {}),
                      content: message.text,
                    }
                  }
                  return { role: message.type, content: message.text }
                }),
              )
              return Promise.resolve({
                structuredResponse: {
                  conclusion: 'A partial conclusion',
                  sources: [],
                },
              })
            }
            const deadlineSignal = options.deadlineSignal
            if (deadlineSignal === undefined) {
              return Promise.reject(new Error('child deadline is missing'))
            }
            return new Promise((resolve) => {
              deadlineSignal.addEventListener(
                'abort',
                () => {
                  resolve({
                    messages: [
                      ...messages,
                      new AIMessage({
                        tool_calls: [
                          {
                            id: 'completed-search-call',
                            name: 'search_web',
                            args: { query: 'completed query' },
                            type: 'tool_call',
                          },
                          {
                            id: 'interrupted-search-call',
                            name: 'search_web',
                            args: { query: 'interrupted query' },
                            type: 'tool_call',
                          },
                        ],
                      }),
                      new ToolMessage({
                        tool_call_id: 'completed-search-call',
                        content: 'Collected partial evidence.',
                      }),
                    ],
                  })
                },
                { once: true },
              )
            })
          },
        }
      },
    }
    const tool = createRunSubagentTool({
      phase,
      parentExecutionStepId: 'parent-step-id',
      recorder: createStepRecorder(() => undefined),
      fetchDefinition: () =>
        okAsync({ ...definition, timeoutMinutes: 1, maxModelCalls: 3 }),
      createMcpClient: () => ({
        getTools: () => Promise.resolve([buildFakeTool('search_web')]),
        close: () => Promise.resolve(),
      }),
      deps: runAgentDeps,
    })

    const resultPromise = tool.invoke({
      key: 'researcher',
      brief: 'Find what can be established.',
    })
    await vi.advanceTimersByTimeAsync(60_001)
    const result = await resultPromise

    const actual = Array.of<unknown>(
      result,
      buildOptions.map((options) => options.systemPrompt),
      buildOptions.map((options) => options.tools.length),
      buildOptions.map((options) => options.agentSignal !== undefined),
      finalizerMessageHistory,
    )

    expect(actual).toEqual([
      { conclusion: 'A partial conclusion', sources: [] },
      [
        'Investigate the question and return evidence.',
        `Investigate the question and return evidence.

制限時間に達しました。これまでに得た情報だけを使って結果を返してください。`,
      ],
      [1, 0],
      [true, false],
      [
        [
          { role: 'human', content: 'Find what can be established.' },
          {
            role: 'assistant',
            toolCalls: [
              {
                id: 'completed-search-call',
                name: 'search_web',
                args: { query: 'completed query' },
              },
              {
                id: 'interrupted-search-call',
                name: 'search_web',
                args: { query: 'interrupted query' },
              },
            ],
          },
          {
            role: 'tool',
            toolCallId: 'completed-search-call',
            content: 'Collected partial evidence.',
          },
          {
            role: 'tool',
            toolCallId: 'interrupted-search-call',
            status: 'error',
            content: '調査用 tool 呼び出しは制限時間で中断されました。',
          },
          {
            role: 'human',
            content:
              '制限時間に達しました。追加調査は行わず、ここまでの情報から指定形式の結果を返してください。',
          },
        ],
      ],
    ])
  })

  it('returns an available partial result when timeout reaches the model-call limit', async () => {
    vi.useFakeTimers()
    const buildOptions: BuildPhaseAgentOptions[] = []
    const tool = createRunSubagentTool({
      phase,
      parentExecutionStepId: 'parent-step-id',
      recorder: createStepRecorder(() => undefined),
      fetchDefinition: () =>
        okAsync({ ...definition, timeoutMinutes: 1, maxModelCalls: 1 }),
      createMcpClient: () => ({
        getTools: () => Promise.resolve([buildFakeTool('search_web')]),
        close: () => Promise.resolve(),
      }),
      deps: {
        createChatModel: () => new FakeChatModel({}),
        buildPhaseAgent: (options): CompiledPhaseAgent => {
          buildOptions.push(options)
          return {
            invoke: ({ messages }) => {
              const budget = options.modelCallBudget
              const signal = options.deadlineSignal
              if (budget === undefined || signal === undefined) {
                return Promise.reject(new Error('child limits are missing'))
              }
              budget.count = budget.maxModelCalls
              return new Promise((resolve) => {
                signal.addEventListener(
                  'abort',
                  () => {
                    resolve({
                      structuredResponse: {
                        conclusion: 'A partial conclusion',
                        sources: [],
                      },
                      messages,
                    })
                  },
                  { once: true },
                )
              })
            },
          }
        },
      },
    })

    const resultPromise = tool.invoke({
      key: 'researcher',
      brief: 'Find what can be established.',
    })
    await vi.advanceTimersByTimeAsync(60_001)
    const result = await resultPromise

    const actual = Array.of<unknown>(result, buildOptions.length)

    expect(actual).toEqual([
      { conclusion: 'A partial conclusion', sources: [] },
      1,
    ])
  })

  it('returns a schema-valid result from messages when timeout uses the model-call limit', async () => {
    vi.useFakeTimers()
    const buildOptions: BuildPhaseAgentOptions[] = []
    const tool = createRunSubagentTool({
      phase,
      parentExecutionStepId: 'parent-step-id',
      recorder: createStepRecorder(() => undefined),
      fetchDefinition: () =>
        okAsync({ ...definition, timeoutMinutes: 1, maxModelCalls: 1 }),
      createMcpClient: () => ({
        getTools: () => Promise.resolve([buildFakeTool('search_web')]),
        close: () => Promise.resolve(),
      }),
      deps: {
        createChatModel: () => new FakeChatModel({}),
        buildPhaseAgent: (options): CompiledPhaseAgent => {
          buildOptions.push(options)
          return {
            invoke: ({ messages }) => {
              const budget = options.modelCallBudget
              const signal = options.deadlineSignal
              if (budget === undefined || signal === undefined) {
                return Promise.reject(new Error('child limits are missing'))
              }
              budget.count = budget.maxModelCalls
              return new Promise((resolve) => {
                signal.addEventListener(
                  'abort',
                  () => {
                    resolve({
                      messages: [
                        ...messages,
                        new AIMessage({
                          tool_calls: [
                            {
                              id: 'partial-output-call',
                              name: 'extract-result',
                              args: {
                                conclusion: 'A partial conclusion',
                                sources: [],
                              },
                              type: 'tool_call',
                            },
                          ],
                        }),
                      ],
                    })
                  },
                  { once: true },
                )
              })
            },
          }
        },
      },
    })

    const resultPromise = tool.invoke({
      key: 'researcher',
      brief: 'Find what can be established.',
    })
    await vi.advanceTimersByTimeAsync(60_001)
    const result = await resultPromise

    const actual = Array.of<unknown>(result, buildOptions.length)

    expect(actual).toEqual([
      { conclusion: 'A partial conclusion', sources: [] },
      1,
    ])
  })

  it('records a failed child step when its definition cannot be fetched', async () => {
    vi.spyOn(crypto, 'randomUUID').mockReturnValue(
      'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
    )
    const notifications: (readonly StrategyTaskStep[])[] = []
    const tool = createRunSubagentTool({
      phase,
      parentExecutionStepId: 'parent-step-id',
      recorder: createStepRecorder((steps) => notifications.push(steps)),
      fetchDefinition: () => errAsync(new Error('definition unavailable')),
      createMcpClient: () => ({
        getTools: () => Promise.resolve([]),
        close: () => Promise.resolve(),
      }),
      deps: {
        createChatModel: () => new FakeChatModel({}),
        buildPhaseAgent: () => ({
          invoke: () => Promise.resolve({ structuredResponse: {} }),
        }),
      },
    })
    const outcome = await tool
      .invoke({
        key: 'researcher',
        brief: 'Investigate the assigned question.',
      })
      .then(
        (value) => ({ status: 'fulfilled' as const, value }),
        (error: unknown) => ({
          status: 'rejected' as const,
          message: error instanceof Error ? error.message : String(error),
        }),
      )
    const actual = Array.of<unknown>(
      outcome,
      normalizeStepTimestamps(notifications.at(-1) ?? []),
    )

    expect(actual).toEqual([
      { status: 'rejected', message: 'definition unavailable' },
      [
        {
          phaseKey: 'subagent:researcher',
          executionStepId: '<execution-step-id-1>',
          label: 'researcher',
          model: 'unknown',
          status: 'failed',
          item: { brief: 'Investigate the assigned question.' },
          startedAt: '<started-at>',
          finishedAt: '<finished-at>',
          traceId: '00000000000000000000000000000000',
          spanId: '0000000000000000',
          error: 'definition unavailable',
        },
      ],
    ])
  })

  it('applies the timeout while loading the child tools', async () => {
    vi.useFakeTimers()
    const buildOptions: BuildPhaseAgentOptions[] = []
    const runAgentDeps: RunAgentGraphDeps = {
      createChatModel: () => new FakeChatModel({}),
      buildPhaseAgent: (options): CompiledPhaseAgent => {
        buildOptions.push(options)
        return {
          invoke: () =>
            Promise.resolve({
              structuredResponse: {
                conclusion: 'No sources were available.',
                sources: [],
              },
            }),
        }
      },
    }
    const tool = createRunSubagentTool({
      phase,
      parentExecutionStepId: 'parent-step-id',
      recorder: createStepRecorder(() => undefined),
      fetchDefinition: () =>
        okAsync({ ...definition, timeoutMinutes: 1, maxModelCalls: 3 }),
      createMcpClient: () => ({
        getTools: () =>
          new Promise((resolve) => {
            setTimeout(() => {
              resolve([])
            }, 120_000)
          }),
        close: () => Promise.resolve(),
      }),
      deps: runAgentDeps,
    })

    const resultPromise = tool.invoke({
      key: 'researcher',
      brief: 'Find what can be established.',
    })
    await vi.advanceTimersByTimeAsync(60_001)
    const result = await resultPromise
    const actual = Array.of<unknown>(
      result,
      buildOptions.map((options) => options.tools.length),
    )

    expect(actual).toEqual([
      { conclusion: 'No sources were available.', sources: [] },
      [0],
    ])
  })
})
