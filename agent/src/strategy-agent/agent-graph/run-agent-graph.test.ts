import { BaseChatModel } from '@langchain/core/language_models/chat_models'
import type { ChatResult } from '@langchain/core/outputs'
import { DynamicStructuredTool } from '@langchain/core/tools'
import { describe, expect, it, vi } from 'vitest'
import { z } from 'zod'

import { logger } from '#logger'
import type {
  BuildPhaseAgentOptions,
  CompiledPhaseAgent,
  RunAgentGraphDeps,
} from '#strategy-agent/agent-graph/run-agent-graph'
import { runAgentGraph } from '#strategy-agent/agent-graph/run-agent-graph'
import { buildPhaseMessageText } from '#strategy-agent/agent-graph/run-agent-graph/phase-message'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import type { AgentGraphConfig } from '#strategy-agent/agent-graph/types'
import type { McpToolsClient } from '#strategy-agent/strategy-agent'
import { createFirstOccurrenceLabeler } from '#test/first-occurrence-labeler'
import { normalizeStepTimestamps } from '#test/normalize-step-timestamps'

// NoopTracer (テスト環境では実 exporter を設定しないため) が返す固定の invalid
// span context。@opentelemetry/api の INVALID_TRACEID/INVALID_SPANID と同じ値。
const NOOP_TRACE_ID = '00000000000000000000000000000000'
const NOOP_SPAN_ID = '0000000000000000'

const STRUCTURED_OUTPUT_TOOL_DESCRIPTION =
  "Tool for extracting structured output from the model's response."

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

const buildStepMcpClientFactory =
  (tools: readonly DynamicStructuredTool[] = []) =>
  (): McpToolsClient => ({
    getTools: () => Promise.resolve([...tools]),
    reserveModelCall: () => Promise.resolve(),
    reserveSubagentRun: () => Promise.resolve(),
    transitionReservation: () => Promise.resolve(),
    close: () => Promise.resolve(),
  })

interface InvokeCall {
  readonly systemPrompt: string
  readonly messageText: string
  readonly executionStepId: string | undefined
}

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/

// executionStepId (crypto.randomUUID()) は実行のたびに変わるため、比較前に
// 出現順で <execution-step-id-N> へ正規化する。同じ値は同じラベルになるため、
// 呼び出し間で値が一致/不一致であることは正規化後も検証できる。
const normalizeExecutionStepIds = (
  calls: readonly InvokeCall[],
): InvokeCall[] => {
  const label = createFirstOccurrenceLabeler('execution-step-id')
  return calls.map((call) => {
    const { executionStepId } = call
    if (executionStepId === undefined) return call
    expect(executionStepId).toMatch(UUID_PATTERN)
    return { ...call, executionStepId: label(executionStepId) }
  })
}

// executionStepId (UUID) は attempt ごとに変わらないが実行のたびにランダムなので、
// warn メッセージ内の UUID を固定プレースホルダーに正規化してから比較する。
const normalizeExecutionStepIdInText = (text: string): string =>
  text.replace(
    /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g,
    '<execution-step-id>',
  )

// この describe 内の複数のテストが再試行経由で logger.warn を出すため、
// spy の設置/解除と正規化をまとめる。
const runWithWarnSpy = async <T>(
  run: () => Promise<T>,
): Promise<{
  readonly result: T
  readonly warnCalls: readonly (readonly [Record<string, unknown>, string])[]
}> => {
  const warnSpy = vi.spyOn(logger, 'warn').mockImplementation(() => undefined)
  try {
    const result = await run()
    return {
      result,
      warnCalls: warnSpy.mock.calls.map(
        ([{ executionStepId, ...fields }, message]) => [
          {
            ...fields,
            executionStepId: normalizeExecutionStepIdInText(
              String(executionStepId),
            ),
          },
          normalizeExecutionStepIdInText(message),
        ],
      ),
    }
  } finally {
    warnSpy.mockRestore()
  }
}

// resume 系テストの previousSteps リテラルから、契約の焦点でない付随フィールドの
// フルスペルを省くためのファクトリ。
const buildPreviousStep = (
  overrides: Partial<StrategyTaskStep> &
    Pick<StrategyTaskStep, 'executionStepId' | 'status'>,
): StrategyTaskStep => ({
  phaseKey: 'investigate',
  label: 'Investigate',
  model: 'm',
  startedAt: '2020-01-01T00:00:00.000Z',
  finishedAt: '2020-01-01T00:00:01.000Z',
  traceId: 'prev-trace',
  spanId: 'prev-span',
  ...overrides,
})

const buildDeps = (
  invokeImpl: (
    call: InvokeCall,
    deadlineSignal: AbortSignal | undefined,
  ) => Promise<{ structuredResponse?: Record<string, unknown> }>,
): { deps: RunAgentGraphDeps; calls: InvokeCall[] } => {
  const calls: InvokeCall[] = []
  const deps: RunAgentGraphDeps = {
    createChatModel: () => new FakeChatModel({}),
    buildPhaseAgent: (options): CompiledPhaseAgent => ({
      invoke: (input) => {
        const call: InvokeCall = {
          systemPrompt: options.systemPrompt,
          messageText: input.messages.map((m) => m.text).join('\n'),
          executionStepId: input.executionStepId,
        }
        calls.push(call)
        return invokeImpl(call, options.deadlineSignal)
      },
    }),
  }
  return { deps, calls }
}

const createDeferred = <T>() => {
  let resolvePromise: (value: T) => void = () => undefined
  const promise = new Promise<T>((resolve) => {
    resolvePromise = resolve
  })
  return { promise, resolve: resolvePromise }
}

const waitForAbort = (
  signal: AbortSignal | undefined,
  onAbort?: () => void,
): Promise<never> =>
  new Promise<never>((_, reject) => {
    const rejectOnAbort = (): void => {
      onAbort?.()
      reject(new Error('phase signal aborted'))
    }
    if (signal?.aborted === true) {
      rejectOnAbort()
    } else {
      signal?.addEventListener('abort', rejectOnAbort, { once: true })
    }
  })

const findAssignedItem = <T extends string>(
  messageText: string,
  items: readonly T[],
  phaseKeys: readonly string[],
): T | undefined =>
  items.find(
    (item) =>
      messageText ===
      buildPhaseMessageText({
        originalPromptText: 'req',
        phaseKeys,
        phaseKey: 'work',
        phasePrompt: 'do work',
        item,
        priorResults: { plan: { items } },
      }),
  )

describe('runAgentGraph', () => {
  it('runs once-phases in order, threading each phase output into later phase messages', async () => {
    const { deps, calls } = buildDeps((call) =>
      Promise.resolve(
        call.messageText.includes('do A')
          ? { structuredResponse: { value: 'A-OUT' } }
          : { structuredResponse: {} },
      ),
    )
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'stepA',
          label: 'Step A',
          model: 'model-a',
          prompt: 'do A',
          subagents: [],
          skills: [],
          tools: [],
          output: { value: { type: 'string' } },
        },
        {
          key: 'stepB',
          label: 'Step B',
          model: 'model-b',
          prompt: 'do B',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'original request',
    })

    expect(result).toEqual({
      status: 'completed',
      message: '2フェーズの実行が完了しました (Step A → Step B)',
    })
    expect(normalizeExecutionStepIds(calls)).toEqual([
      {
        systemPrompt: 'AGENTS',
        messageText: buildPhaseMessageText({
          originalPromptText: 'original request',
          phaseKeys: ['stepA', 'stepB'],
          phaseKey: 'stepA',
          phasePrompt: 'do A',
          item: undefined,
          priorResults: {},
        }),
        executionStepId: '<execution-step-id-1>',
      },
      {
        systemPrompt: 'AGENTS',
        messageText: buildPhaseMessageText({
          originalPromptText: 'original request',
          phaseKeys: ['stepA', 'stepB'],
          phaseKey: 'stepB',
          phasePrompt: 'do B',
          item: undefined,
          priorResults: { stepA: { value: 'A-OUT' } },
        }),
        executionStepId: '<execution-step-id-2>',
      },
    ])
  })

  it('adds the subagent tool alongside phase-scoped MCP tools', async () => {
    const configuredTools: string[][] = []
    const factoryCalls: string[] = []
    const deps: RunAgentGraphDeps = {
      createChatModel: () => new FakeChatModel({}),
      buildPhaseAgent: (options): CompiledPhaseAgent => {
        configuredTools.push(options.tools.map((tool) => tool.name))
        return {
          invoke: () => Promise.resolve({ structuredResponse: {} }),
        }
      },
    }
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'work',
          label: 'Work',
          model: 'sample-model',
          prompt: 'do work',
          subagents: ['researcher'],
          skills: [],
          tools: ['search_web'],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory([
        buildFakeTool('search_web'),
        buildFakeTool('read_page'),
      ]),
      createSubagentTool: (phase) => {
        factoryCalls.push(phase.key)
        return buildFakeTool('run_subagent')
      },
      originalPromptText: 'req',
    })

    const actual = Array.of<unknown>(result, configuredTools, factoryCalls)

    expect(actual).toEqual([
      {
        status: 'completed',
        message: '1フェーズの実行が完了しました (Work)',
      },
      [['search_web', 'run_subagent']],
      ['work'],
    ])
  })

  it('preserves legacy LLM tools while keeping task runtime tools internal', async () => {
    const configuredTools: string[][] = []
    const deps: RunAgentGraphDeps = {
      createChatModel: () => new FakeChatModel({}),
      buildPhaseAgent: (options): CompiledPhaseAgent => {
        configuredTools.push(options.tools.map((tool) => tool.name))
        return {
          invoke: () => Promise.resolve({ structuredResponse: {} }),
        }
      },
    }
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'work',
          label: 'Work',
          model: 'demo-model',
          prompt: 'Process the synthetic input.',
          skills: [],
          subagents: [],
          tools: [
            'execute_operation',
            'read_execution_payload',
            'capture_dataset',
            'read_task_execution_snapshot',
          ],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory([
        buildFakeTool('execute_operation'),
        buildFakeTool('read_execution_payload'),
        buildFakeTool('capture_dataset'),
        buildFakeTool('read_task_execution_snapshot'),
      ]),
      originalPromptText: 'req',
    })

    const actual = Array.of<unknown>(result, configuredTools)

    expect(actual).toEqual([
      {
        status: 'completed',
        message: '1フェーズの実行が完了しました (Work)',
      },
      [['execute_operation', 'read_execution_payload']],
    ])
  })

  it('fails immediately without running any phase when deadlineSignal is already aborted', async () => {
    const { deps, calls } = buildDeps(() =>
      Promise.reject(new Error('phase should not be invoked')),
    )
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'stepA',
          label: 'Step A',
          model: 'model-a',
          prompt: 'do A',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
      deadlineSignal: AbortSignal.abort(),
    })

    expect(result).toEqual({
      status: 'failed',
      message:
        'フェーズ「Step A」(stepA) の実行に失敗しました: deadline を超過したため実行を中断しました',
      errorKind: 'agent_error',
    })
    expect(calls).toEqual([])
  })

  it('preserves the original usage limit error when an unconfigured phase hits the deadline', async () => {
    const controller = new AbortController()
    const usageLimitError = Object.assign(new Error('usage limit exceeded'), {
      rateLimitType: 'capacity',
    })
    const { deps } = buildDeps(() => {
      controller.abort()
      return Promise.reject(usageLimitError)
    })

    const result = await runAgentGraph(
      deps,
      {
        phases: [
          {
            key: 'work',
            label: 'Work',
            model: 'example-model',
            prompt: 'do work',
            subagents: [],
            skills: [],
            tools: [],
            output: {},
          },
        ],
      },
      {
        agentsMd: 'AGENTS',
        skills: {},
        createStepMcpClient: buildStepMcpClientFactory(),
        originalPromptText: 'req',
        deadlineSignal: controller.signal,
      },
    )

    expect(result).toEqual({
      status: 'failed',
      message:
        'フェーズ「Work」(work) の実行に失敗しました: usage limit exceeded',
      errorKind: 'usage_limit',
    })
  })

  it('preserves the usage limit classification when a configured phase hits the task deadline', async () => {
    const controller = new AbortController()
    const usageLimitError = Object.assign(new Error('usage limit exceeded'), {
      rateLimitType: 'capacity',
    })
    const { deps } = buildDeps(() => {
      controller.abort()
      return Promise.reject(usageLimitError)
    })

    const result = await runAgentGraph(
      deps,
      {
        phases: [
          {
            key: 'work',
            label: 'Work',
            model: 'example-model',
            prompt: 'do work',
            timeoutMinutes: 1,
            subagents: [],
            skills: [],
            tools: [],
            output: {},
          },
        ],
      },
      {
        agentsMd: 'AGENTS',
        skills: {},
        createStepMcpClient: buildStepMcpClientFactory(),
        originalPromptText: 'req',
        deadlineSignal: controller.signal,
      },
    )

    expect(result).toEqual({
      status: 'failed',
      message:
        'フェーズ「Work」(work) の実行に失敗しました: deadline を超過したため実行を中断しました',
      errorKind: 'usage_limit',
    })
  })

  it('does not start pending for_each items after the deadline is exceeded', async () => {
    const controller = new AbortController()
    const { deps, calls } = buildDeps((call) => {
      if (call.messageText.includes('do plan')) {
        return Promise.resolve({
          structuredResponse: { items: ['a', 'b', 'c', 'd'] },
        })
      }
      controller.abort()
      return Promise.resolve({ structuredResponse: {} })
    })
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'plan',
          label: 'Plan',
          model: 'm',
          prompt: 'do plan',
          subagents: [],
          skills: [],
          tools: [],
          output: { items: { type: 'array', items: { type: 'string' } } },
        },
        {
          key: 'work',
          label: 'Work',
          model: 'm',
          prompt: 'do work',
          forEach: 'plan.items',
          maxParallel: 2,
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
      deadlineSignal: controller.signal,
    })

    const actual = Array.of<unknown>(result, normalizeExecutionStepIds(calls))
    expect(actual).toEqual([
      {
        status: 'failed',
        message:
          'フェーズ「Work」(work) の実行に失敗しました: deadline を超過したため実行を中断しました',
        errorKind: 'agent_error',
      },
      [
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'work'],
            phaseKey: 'plan',
            phasePrompt: 'do plan',
            item: undefined,
            priorResults: {},
          }),
          executionStepId: '<execution-step-id-1>',
        },
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'work'],
            phaseKey: 'work',
            phasePrompt: 'do work',
            item: 'a',
            priorResults: { plan: { items: ['a', 'b', 'c', 'd'] } },
          }),
          executionStepId: '<execution-step-id-2>',
        },
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'work'],
            phaseKey: 'work',
            phasePrompt: 'do work',
            item: 'b',
            priorResults: { plan: { items: ['a', 'b', 'c', 'd'] } },
          }),
          executionStepId: '<execution-step-id-3>',
        },
      ],
    ])
  })

  it('keeps the task deadline reason when it wins over the phase timeout', async () => {
    const controller = new AbortController()
    const { deps, calls } = buildDeps((call) => {
      if (call.messageText.includes('do plan')) {
        return Promise.resolve({
          structuredResponse: { items: ['a', 'b', 'c', 'd'] },
        })
      }
      controller.abort()
      return Promise.resolve({ structuredResponse: {} })
    })
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'plan',
          label: 'Plan',
          model: 'm',
          prompt: 'do plan',
          subagents: [],
          skills: [],
          tools: [],
          output: { items: { type: 'array', items: { type: 'string' } } },
        },
        {
          key: 'work',
          label: 'Work',
          model: 'm',
          prompt: 'do work',
          forEach: 'plan.items',
          maxParallel: 2,
          timeoutMinutes: 1,
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
      deadlineSignal: controller.signal,
    })

    const actual = Array.of<unknown>(result, normalizeExecutionStepIds(calls))
    expect(actual).toEqual([
      {
        status: 'failed',
        message:
          'フェーズ「Work」(work) の実行に失敗しました: deadline を超過したため実行を中断しました',
        errorKind: 'agent_error',
      },
      [
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'work'],
            phaseKey: 'plan',
            phasePrompt: 'do plan',
            item: undefined,
            priorResults: {},
          }),
          executionStepId: '<execution-step-id-1>',
        },
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'work'],
            phaseKey: 'work',
            phasePrompt: 'do work',
            item: 'a',
            priorResults: { plan: { items: ['a', 'b', 'c', 'd'] } },
          }),
          executionStepId: '<execution-step-id-2>',
        },
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'work'],
            phaseKey: 'work',
            phasePrompt: 'do work',
            item: 'b',
            priorResults: { plan: { items: ['a', 'b', 'c', 'd'] } },
          }),
          executionStepId: '<execution-step-id-3>',
        },
      ],
    ])
  })

  it('keeps partial for_each output when a task deadline aborts the last item', async () => {
    const controller = new AbortController()
    let workCalls = 0
    const { deps, calls } = buildDeps((call) => {
      if (call.messageText.includes('do plan')) {
        return Promise.resolve({
          structuredResponse: { items: ['a', 'b'] },
        })
      }
      workCalls += 1
      if (workCalls === 1) {
        return Promise.resolve({ structuredResponse: { value: 'done' } })
      }
      controller.abort()
      return Promise.reject(new Error('aborted'))
    })
    const result = await runAgentGraph(
      deps,
      {
        phases: [
          {
            key: 'plan',
            label: 'Plan',
            model: 'example-model-plan',
            prompt: 'do plan',
            subagents: [],
            skills: [],
            output: { items: { type: 'array', items: { type: 'string' } } },
          },
          {
            key: 'work',
            label: 'Work',
            model: 'example-model-work',
            prompt: 'do work',
            forEach: 'plan.items',
            maxParallel: 1,
            subagents: [],
            skills: [],
            output: {},
          },
        ],
      },
      {
        agentsMd: 'AGENTS',
        skills: {},
        createStepMcpClient: buildStepMcpClientFactory(),
        originalPromptText: 'req',
        deadlineSignal: controller.signal,
      },
    )

    expect(Array.of<unknown>(result, calls.length)).toEqual([
      {
        status: 'completed',
        message: '2フェーズの実行が完了しました (Plan → Work)',
      },
      3,
    ])
  })

  it('applies one timeout to the whole for_each phase', async () => {
    vi.useFakeTimers()
    try {
      const firstItemStarted = createDeferred<undefined>()
      const secondItemStarted = createDeferred<undefined>()
      const calls: string[] = []
      let workCalls = 0
      const { deps } = buildDeps((call, deadlineSignal) => {
        if (call.messageText.includes('build items')) {
          calls.push('plan')
          return Promise.resolve({
            structuredResponse: { items: ['first', 'second'] },
          })
        }

        calls.push('work')
        workCalls += 1
        if (workCalls === 1) firstItemStarted.resolve(undefined)
        if (workCalls === 2) secondItemStarted.resolve(undefined)
        const itemResult = createDeferred<{
          structuredResponse?: Record<string, unknown>
        }>()
        const itemTimer = setTimeout(() => {
          itemResult.resolve({ structuredResponse: {} })
        }, 40_000)
        return Promise.race([
          itemResult.promise,
          waitForAbort(deadlineSignal, () => {
            clearTimeout(itemTimer)
          }),
        ])
      })
      const execution = runAgentGraph(
        deps,
        {
          phases: [
            {
              key: 'plan',
              label: 'Plan',
              model: 'example-model-plan',
              prompt: 'build items',
              subagents: [],
              skills: [],
              output: {
                items: { type: 'array', items: { type: 'string' } },
              },
            },
            {
              key: 'work',
              label: 'Work',
              model: 'example-model-work',
              prompt: 'process item',
              forEach: 'plan.items',
              maxParallel: 1,
              timeoutMinutes: 1,
              subagents: [],
              skills: [],
              output: {},
            },
          ],
        },
        {
          agentsMd: 'AGENTS',
          skills: {},
          createStepMcpClient: buildStepMcpClientFactory(),
          originalPromptText: 'req',
        },
      )

      await firstItemStarted.promise
      await vi.advanceTimersByTimeAsync(40_000)
      await secondItemStarted.promise
      await vi.advanceTimersByTimeAsync(20_000)

      const actual = Array.of<unknown>(await execution, calls)
      expect(actual).toEqual([
        {
          status: 'failed',
          message: 'フェーズ work が timeout (1 分) を超過',
          errorKind: 'agent_error',
        },
        ['plan', 'work', 'work'],
      ])
    } finally {
      vi.useRealTimers()
    }
  })

  it('aborts every running for_each item when its phase timeout expires', async () => {
    vi.useFakeTimers()
    try {
      const itemsStarted = createDeferred<undefined>()
      const items = ['first', 'second', 'pending']
      const startedItems: string[] = []
      const abortedItems: string[] = []
      const { deps } = buildDeps((call, deadlineSignal) => {
        if (call.messageText.includes('build items')) {
          return Promise.resolve({ structuredResponse: { items } })
        }

        const item = findAssignedItem(call.messageText, items, ['plan', 'work'])
        if (item === undefined) {
          return Promise.reject(new Error('unexpected item'))
        }
        startedItems.push(item)
        if (startedItems.length === 2) itemsStarted.resolve(undefined)
        return waitForAbort(deadlineSignal, () => {
          abortedItems.push(item)
        })
      })
      const execution = runAgentGraph(
        deps,
        {
          phases: [
            {
              key: 'plan',
              label: 'Plan',
              model: 'example-model-plan',
              prompt: 'build items',
              subagents: [],
              skills: [],
              output: {
                items: { type: 'array', items: { type: 'string' } },
              },
            },
            {
              key: 'work',
              label: 'Work',
              model: 'example-model-work',
              prompt: 'do work',
              forEach: 'plan.items',
              maxParallel: 2,
              timeoutMinutes: 1,
              subagents: [],
              skills: [],
              output: {},
            },
          ],
        },
        {
          agentsMd: 'AGENTS',
          skills: {},
          createStepMcpClient: buildStepMcpClientFactory(),
          originalPromptText: 'req',
        },
      )

      await itemsStarted.promise
      await vi.advanceTimersByTimeAsync(60_000)

      const actual = Array.of<unknown>(
        await execution,
        startedItems,
        abortedItems,
      )
      expect(actual).toEqual([
        {
          status: 'failed',
          message: 'フェーズ work が timeout (1 分) を超過',
          errorKind: 'agent_error',
        },
        ['first', 'second'],
        ['first', 'second'],
      ])
    } finally {
      vi.useRealTimers()
    }
  })

  it('starts a phase timeout when that phase begins after a long prior phase', async () => {
    vi.useFakeTimers()
    try {
      const priorPhaseStarted = createDeferred<undefined>()
      const timedPhaseStarted = createDeferred<undefined>()
      const phaseStartTimes: number[] = []
      const abortTimes: number[] = []
      const { deps } = buildDeps((call, deadlineSignal) => {
        if (call.messageText.includes('do prior work')) {
          phaseStartTimes.push(Date.now())
          priorPhaseStarted.resolve(undefined)
          return new Promise((resolve) => {
            setTimeout(() => {
              resolve({ structuredResponse: { value: 'done' } })
            }, 90_000)
          })
        }

        phaseStartTimes.push(Date.now())
        timedPhaseStarted.resolve(undefined)
        return waitForAbort(deadlineSignal, () => {
          abortTimes.push(Date.now())
        })
      })
      const graphStartedAt = Date.now()
      const execution = runAgentGraph(
        deps,
        {
          phases: [
            {
              key: 'prior',
              label: 'Prior',
              model: 'example-model-prior',
              prompt: 'do prior work',
              subagents: [],
              skills: [],
              output: { value: { type: 'string' } },
            },
            {
              key: 'timed',
              label: 'Timed',
              model: 'example-model-timed',
              prompt: 'do timed work',
              timeoutMinutes: 1,
              subagents: [],
              skills: [],
              output: {},
            },
          ],
        },
        {
          agentsMd: 'AGENTS',
          skills: {},
          createStepMcpClient: buildStepMcpClientFactory(),
          originalPromptText: 'req',
        },
      )

      await priorPhaseStarted.promise
      await vi.advanceTimersByTimeAsync(90_000)
      await timedPhaseStarted.promise
      await vi.advanceTimersByTimeAsync(59_999)
      const wasAbortedBeforeTimeout = abortTimes.length > 0
      await vi.advanceTimersByTimeAsync(1)

      const timedPhaseStartedAt = phaseStartTimes[1]
      const phaseAbortedAt = abortTimes[0]
      const actual = Array.of<unknown>(
        await execution,
        phaseStartTimes.map((time) => time - graphStartedAt),
        phaseAbortedAt === undefined || timedPhaseStartedAt === undefined
          ? undefined
          : phaseAbortedAt - timedPhaseStartedAt,
        wasAbortedBeforeTimeout,
      )
      expect(actual).toEqual([
        {
          status: 'failed',
          message: 'フェーズ timed が timeout (1 分) を超過',
          errorKind: 'agent_error',
        },
        [0, 90_000],
        60_000,
        false,
      ])
    } finally {
      vi.useRealTimers()
    }
  })

  it('passes the phase reasoning effort through to createChatModel, omitting it when unset', async () => {
    const createChatModelCalls: {
      model: string
      options: { reasoningEffort?: string } | undefined
    }[] = []
    const deps: RunAgentGraphDeps = {
      createChatModel: (model, options) => {
        createChatModelCalls.push({ model, options })
        return new FakeChatModel({})
      },
      buildPhaseAgent: (): CompiledPhaseAgent => ({
        invoke: () => Promise.resolve({ structuredResponse: {} }),
      }),
    }
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'withEffort',
          label: 'With effort',
          model: 'model-a',
          reasoningEffort: 'high',
          prompt: 'do a',
          subagents: [],
          skills: [],
          output: {},
        },
        {
          key: 'withoutEffort',
          label: 'Without effort',
          model: 'model-b',
          prompt: 'do b',
          subagents: [],
          skills: [],
          output: {},
        },
      ],
    }

    await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
    })

    expect(createChatModelCalls).toEqual([
      { model: 'model-a', options: { reasoningEffort: 'high' } },
      { model: 'model-b', options: {} },
    ])
  })

  it('filters tools and skills down to what the phase declares', async () => {
    let capturedOptions: BuildPhaseAgentOptions | undefined
    let chatModelReturnValue: BaseChatModel | undefined
    const deps: RunAgentGraphDeps = {
      createChatModel: () => {
        const model = new FakeChatModel({})
        chatModelReturnValue = model
        return model
      },
      buildPhaseAgent: (options): CompiledPhaseAgent => {
        capturedOptions = options
        return { invoke: () => Promise.resolve({ structuredResponse: {} }) }
      },
    }
    const toolA = buildFakeTool('tool_a')
    const toolB = buildFakeTool('tool_b')
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'p',
          label: 'P',
          model: 'm',
          prompt: 'do p',
          subagents: [],
          skills: ['skill-a'],
          tools: ['tool_a'],
          output: {},
        },
      ],
    }

    await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: { 'skill-a': 'body a', 'skill-b': 'body b' },
      createStepMcpClient: buildStepMcpClientFactory([toolA, toolB]),
      originalPromptText: 'req',
    })

    const capturedOptionSnapshot =
      capturedOptions === undefined
        ? undefined
        : {
            ...capturedOptions,
            reserveModelCall: typeof capturedOptions.reserveModelCall,
            transitionReservation: typeof capturedOptions.transitionReservation,
            onModelCallLimitReached:
              typeof capturedOptions.onModelCallLimitReached,
          }

    expect(capturedOptionSnapshot).toEqual({
      model: chatModelReturnValue,
      tools: [toolA],
      mcpTools: [toolA],
      reserveModelCall: 'function',
      transitionReservation: 'function',
      onModelCallLimitReached: 'function',
      systemPrompt: 'AGENTS\n\n# Skill: skill-a\n\nbody a',
      responseSchema: {
        type: 'object',
        properties: {},
        description: STRUCTURED_OUTPUT_TOOL_DESCRIPTION,
      },
    })
  })

  it('passes through every tool when the phase omits tools', async () => {
    let capturedOptions: BuildPhaseAgentOptions | undefined
    let chatModelReturnValue: BaseChatModel | undefined
    const deps: RunAgentGraphDeps = {
      createChatModel: () => {
        const model = new FakeChatModel({})
        chatModelReturnValue = model
        return model
      },
      buildPhaseAgent: (options): CompiledPhaseAgent => {
        capturedOptions = options
        return { invoke: () => Promise.resolve({ structuredResponse: {} }) }
      },
    }
    const toolA = buildFakeTool('tool_a')
    const toolB = buildFakeTool('tool_b')
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'p',
          label: 'P',
          model: 'm',
          prompt: 'do p',
          subagents: [],
          skills: [],
          output: {},
        },
      ],
    }

    await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory([toolA, toolB]),
      originalPromptText: 'req',
    })

    const capturedOptionSnapshot =
      capturedOptions === undefined
        ? undefined
        : {
            ...capturedOptions,
            reserveModelCall: typeof capturedOptions.reserveModelCall,
            transitionReservation: typeof capturedOptions.transitionReservation,
            onModelCallLimitReached:
              typeof capturedOptions.onModelCallLimitReached,
          }

    expect(capturedOptionSnapshot).toEqual({
      model: chatModelReturnValue,
      tools: [toolA, toolB],
      mcpTools: [toolA, toolB],
      reserveModelCall: 'function',
      transitionReservation: 'function',
      onModelCallLimitReached: 'function',
      systemPrompt: 'AGENTS',
      responseSchema: {
        type: 'object',
        properties: {},
        description: STRUCTURED_OUTPUT_TOOL_DESCRIPTION,
      },
    })
  })

  it('caps for_each concurrency at max_parallel', async () => {
    let active = 0
    let maxActive = 0
    const { deps } = buildDeps(async (call) => {
      if (call.messageText.includes('do plan')) {
        return { structuredResponse: { items: ['a', 'b', 'c', 'd', 'e'] } }
      }
      active++
      maxActive = Math.max(maxActive, active)
      await new Promise((resolve) => setTimeout(resolve, 10))
      active--
      return { structuredResponse: {} }
    })
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'plan',
          label: 'Plan',
          model: 'm',
          prompt: 'do plan',
          subagents: [],
          skills: [],
          tools: [],
          output: { items: { type: 'array', items: { type: 'string' } } },
        },
        {
          key: 'work',
          label: 'Work',
          model: 'm',
          prompt: 'do work',
          forEach: 'plan.items',
          maxParallel: 2,
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
    })

    expect(result).toEqual({
      status: 'completed',
      message: '2フェーズの実行が完了しました (Plan → Work)',
    })
    expect(maxActive).toBe(2)
  })

  it('starts the next for_each item when a worker becomes available', async () => {
    const items = ['item-one', 'item-two', 'item-three', 'item-four'] as const
    const completions = new Map(
      items.map(
        (item) =>
          [
            item,
            createDeferred<{ structuredResponse: Record<string, unknown> }>(),
          ] as const,
      ),
    )
    const started: string[] = []
    const twoStarted = createDeferred<undefined>()
    const threeStarted = createDeferred<undefined>()
    const { deps } = buildDeps((call) => {
      if (call.messageText.includes('do plan')) {
        return Promise.resolve({ structuredResponse: { items } })
      }
      const item = findAssignedItem(call.messageText, items, ['plan', 'work'])
      if (item === undefined) {
        return Promise.resolve({ structuredResponse: {} })
      }
      started.push(item)
      if (started.length === 2) twoStarted.resolve(undefined)
      if (started.length === 3) threeStarted.resolve(undefined)
      return (
        completions.get(item)?.promise ??
        Promise.resolve({ structuredResponse: { value: item } })
      )
    })
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'plan',
          label: 'Plan',
          model: 'm',
          prompt: 'do plan',
          subagents: [],
          skills: [],
          tools: [],
          output: { items: { type: 'array', items: { type: 'string' } } },
        },
        {
          key: 'work',
          label: 'Work',
          model: 'm',
          prompt: 'do work',
          forEach: 'plan.items',
          maxParallel: 2,
          subagents: [],
          skills: [],
          tools: [],
          output: { value: { type: 'string' } },
        },
      ],
    }

    const runPromise = runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
    })
    await twoStarted.promise
    completions
      .get(items[1])
      ?.resolve({ structuredResponse: { value: items[1] } })
    const nextItemStartedBeforeFirstCompleted = await Promise.race([
      threeStarted.promise.then(() => true),
      new Promise<boolean>((resolve) => {
        setTimeout(() => {
          resolve(false)
        }, 0)
      }),
    ])
    for (const item of items) {
      completions.get(item)?.resolve({ structuredResponse: { value: item } })
    }
    const result = await runPromise

    const actual = Array.of<unknown>(
      started,
      nextItemStartedBeforeFirstCompleted,
      result,
    )
    expect(actual).toEqual([
      items,
      true,
      {
        status: 'completed',
        message: '2フェーズの実行が完了しました (Plan → Work)',
      },
    ])
  })

  it('passes for_each outputs in item order when workers complete out of order', async () => {
    const items = ['item-one', 'item-two', 'item-three']
    const completions = new Map(
      items.map(
        (item) =>
          [
            item,
            createDeferred<{ structuredResponse: Record<string, unknown> }>(),
          ] as const,
      ),
    )
    const allStarted = createDeferred<undefined>()
    let startedCount = 0
    const { deps, calls } = buildDeps((call) => {
      if (call.messageText.includes('make plan')) {
        return Promise.resolve({ structuredResponse: { items } })
      }
      if (call.messageText.includes('do work')) {
        const item = findAssignedItem(call.messageText, items, [
          'plan',
          'work',
          'summary',
        ])
        if (item === undefined) {
          return Promise.resolve({ structuredResponse: {} })
        }
        startedCount++
        if (startedCount === items.length) allStarted.resolve(undefined)
        return (
          completions.get(item)?.promise ??
          Promise.resolve({ structuredResponse: { value: item } })
        )
      }
      return Promise.resolve({ structuredResponse: {} })
    })
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'plan',
          label: 'Plan',
          model: 'm',
          prompt: 'make plan',
          subagents: [],
          skills: [],
          tools: [],
          output: { items: { type: 'array', items: { type: 'string' } } },
        },
        {
          key: 'work',
          label: 'Work',
          model: 'm',
          prompt: 'do work',
          forEach: 'plan.items',
          maxParallel: items.length,
          subagents: [],
          skills: [],
          tools: [],
          output: { value: { type: 'string' } },
        },
        {
          key: 'summary',
          label: 'Summary',
          model: 'm',
          prompt: 'summarize',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const runPromise = runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
    })
    await allStarted.promise
    for (const item of [...items].reverse()) {
      completions.get(item)?.resolve({ structuredResponse: { value: item } })
    }
    await runPromise

    expect(normalizeExecutionStepIds(calls.slice(-1))).toEqual([
      {
        systemPrompt: 'AGENTS',
        messageText: buildPhaseMessageText({
          originalPromptText: 'req',
          phaseKeys: ['plan', 'work', 'summary'],
          phaseKey: 'summary',
          phasePrompt: 'summarize',
          item: undefined,
          priorResults: {
            plan: { items },
            work: items.map((item) => ({ value: item })),
          },
        }),
        executionStepId: '<execution-step-id-1>',
      },
    ])
  })

  it('retries a phase up to 2 times after a missing structured response, then succeeds, reusing one executionStepId across attempts', async () => {
    let attempts = 0
    const { deps, calls } = buildDeps(() => {
      attempts++
      return Promise.resolve(
        attempts < 3 ? {} : { structuredResponse: { ok: true } },
      )
    })
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'p',
          label: 'P',
          model: 'm',
          prompt: 'do p',
          subagents: [],
          skills: [],
          tools: [],
          output: { ok: { type: 'boolean' } },
        },
      ],
    }

    const { result, warnCalls } = await runWithWarnSpy(() =>
      runAgentGraph(deps, config, {
        agentsMd: 'AGENTS',
        skills: {},
        createStepMcpClient: buildStepMcpClientFactory(),
        originalPromptText: 'req',
      }),
    )

    expect(result).toEqual({
      status: 'completed',
      message: '1フェーズの実行が完了しました (P)',
    })
    expect(attempts).toBe(3)
    expect(
      normalizeExecutionStepIds(calls).map((call) => call.executionStepId),
    ).toEqual([
      '<execution-step-id-1>',
      '<execution-step-id-1>',
      '<execution-step-id-1>',
    ])
    expect(warnCalls).toEqual([
      [
        {
          executionStepId: '<execution-step-id>',
          attemptNumber: 1,
          reason: 'missing_structured_output',
        },
        'phase "p" (executionStepId=<execution-step-id>) structured output attempt 1 rejected (missing_structured_output), retrying',
      ],
      [
        {
          executionStepId: '<execution-step-id>',
          attemptNumber: 2,
          reason: 'missing_structured_output',
        },
        'phase "p" (executionStepId=<execution-step-id>) structured output attempt 2 rejected (missing_structured_output), retrying',
      ],
    ])

    const basePrompt = buildPhaseMessageText({
      originalPromptText: 'req',
      phaseKeys: ['p'],
      phaseKey: 'p',
      phasePrompt: 'do p',
      item: undefined,
      priorResults: {},
    })
    const feedback = [
      '前回の試行は却下されました (理由: agent did not return a structured response)。',
      '構造化出力 tool を呼び出して、必須フィールドを満たす内容を提出し直してください。',
      'write_note・create_annotation などの書き込み系 tool は前回の試行で既に実行済みの可能性があります。同じ内容を重複して実行しないでください。',
    ].join('\n')
    expect(calls.map((call) => call.messageText)).toEqual([
      basePrompt,
      [basePrompt, feedback].join('\n'),
      [basePrompt, feedback, feedback].join('\n'),
    ])
  })

  it.each([
    {
      name: 'missing structured output',
      firstAttemptResult: {},
      reason: 'missing_structured_output',
      reasonText: 'agent did not return a structured response',
    },
    {
      name: 'missing required array field',
      firstAttemptResult: { structuredResponse: {} },
      reason: 'missing_required_array_fields',
      reasonText:
        "agent's structured response did not resolve required for_each field(s) to an array: ok",
    },
  ])(
    'feeds back the rejection reason and a write-tool warning on retry ($name)',
    async ({ firstAttemptResult, reason, reasonText }) => {
      let planAttempts = 0
      const { deps, calls } = buildDeps((call) => {
        if (!call.messageText.includes('do plan')) {
          return Promise.resolve({ structuredResponse: {} })
        }
        planAttempts++
        return Promise.resolve(
          planAttempts === 1
            ? firstAttemptResult
            : { structuredResponse: { ok: ['done'] } },
        )
      })
      const config: AgentGraphConfig = {
        phases: [
          {
            key: 'plan',
            label: 'Plan',
            model: 'm',
            prompt: 'do plan',
            subagents: [],
            skills: [],
            tools: [],
            output: { ok: { type: 'array', items: { type: 'string' } } },
          },
          {
            key: 'work',
            label: 'Work',
            model: 'm',
            prompt: 'do work',
            forEach: 'plan.ok',
            subagents: [],
            skills: [],
            tools: [],
            output: {},
          },
        ],
      }

      const { warnCalls } = await runWithWarnSpy(() =>
        runAgentGraph(deps, config, {
          agentsMd: 'AGENTS',
          skills: {},
          createStepMcpClient: buildStepMcpClientFactory(),
          originalPromptText: 'req',
        }),
      )

      const basePrompt = buildPhaseMessageText({
        originalPromptText: 'req',
        phaseKeys: ['plan', 'work'],
        phaseKey: 'plan',
        phasePrompt: 'do plan',
        item: undefined,
        priorResults: {},
      })
      const planMessageTexts = calls
        .filter((call) => call.messageText.startsWith(basePrompt))
        .map((call) => call.messageText)

      expect(planMessageTexts).toEqual([
        basePrompt,
        [
          basePrompt,
          `前回の試行は却下されました (理由: ${reasonText})。\n構造化出力 tool を呼び出して、必須フィールドを満たす内容を提出し直してください。\nwrite_note・create_annotation などの書き込み系 tool は前回の試行で既に実行済みの可能性があります。同じ内容を重複して実行しないでください。`,
        ].join('\n'),
      ])
      expect(warnCalls).toEqual([
        [
          { executionStepId: '<execution-step-id>', attemptNumber: 1, reason },
          `phase "plan" (executionStepId=<execution-step-id>) structured output attempt 1 rejected (${reason}), retrying`,
        ],
      ])
    },
  )

  it('fails the phase after exhausting all structured-output retries', async () => {
    let attempts = 0
    const { deps } = buildDeps(() => {
      attempts++
      return Promise.resolve({})
    })
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'p',
          label: 'P',
          model: 'm',
          prompt: 'do p',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const { result, warnCalls } = await runWithWarnSpy(() =>
      runAgentGraph(deps, config, {
        agentsMd: 'AGENTS',
        skills: {},
        createStepMcpClient: buildStepMcpClientFactory(),
        originalPromptText: 'req',
      }),
    )

    expect(result).toEqual({
      status: 'failed',
      message:
        'フェーズ「P」(p) の実行に失敗しました: agent did not return a structured response',
      errorKind: 'agent_error',
    })
    expect(attempts).toBe(3)
    expect(warnCalls).toEqual([
      [
        {
          executionStepId: '<execution-step-id>',
          attemptNumber: 1,
          reason: 'missing_structured_output',
        },
        'phase "p" (executionStepId=<execution-step-id>) structured output attempt 1 rejected (missing_structured_output), retrying',
      ],
      [
        {
          executionStepId: '<execution-step-id>',
          attemptNumber: 2,
          reason: 'missing_structured_output',
        },
        'phase "p" (executionStepId=<execution-step-id>) structured output attempt 2 rejected (missing_structured_output), retrying',
      ],
    ])
  })

  it('propagates an invoke rejection immediately, without retrying', async () => {
    let attempts = 0
    const { deps } = buildDeps(() => {
      attempts++
      return Promise.reject(new Error('usage limit exceeded'))
    })
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'p',
          label: 'P',
          model: 'm',
          prompt: 'do p',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
    })

    expect(result).toEqual({
      status: 'failed',
      message: 'フェーズ「P」(p) の実行に失敗しました: usage limit exceeded',
      errorKind: 'agent_error',
    })
    expect(attempts).toBe(1)
  })

  it('records the step as failed when the step MCP client fails to fetch tools', async () => {
    let attempts = 0
    const { deps } = buildDeps(() => {
      attempts++
      return Promise.resolve({ structuredResponse: {} })
    })
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'p',
          label: 'P',
          model: 'm',
          prompt: 'do p',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: () => ({
        getTools: () => Promise.reject(new Error('cannot connect')),
        reserveModelCall: () => Promise.resolve(),
        reserveSubagentRun: () => Promise.resolve(),
        transitionReservation: () => Promise.resolve(),
        close: () => Promise.resolve(),
      }),
      originalPromptText: 'req',
    })

    expect(result).toEqual({
      status: 'failed',
      message: 'フェーズ「P」(p) の実行に失敗しました: cannot connect',
      errorKind: 'agent_error',
    })
    expect(attempts).toBe(0)
  })

  it.each([
    {
      name: 'never becomes an array',
      output: { value: { type: 'string' } },
      forEach: 'plan.value',
      response: { value: 'not-an-array' },
      field: 'value',
    },
    {
      name: 'is missing',
      output: { hypotheses: { type: 'array' } },
      forEach: 'plan.hypotheses',
      response: {},
      field: 'hypotheses',
    },
  ])(
    'retries and then fails the source phase when for_each references a field that $name',
    async ({ output, forEach, response, field }) => {
      let attempts = 0
      const { deps } = buildDeps(() => {
        attempts++
        return Promise.resolve({ structuredResponse: response })
      })
      const config: AgentGraphConfig = {
        phases: [
          {
            key: 'plan',
            label: 'Plan',
            model: 'm',
            prompt: 'do plan',
            subagents: [],
            skills: [],
            tools: [],
            output,
          },
          {
            key: 'investigate',
            label: 'Investigate',
            model: 'm',
            prompt: 'do investigate',
            forEach,
            subagents: [],
            skills: [],
            tools: [],
            output: {},
          },
        ],
      }

      const { result, warnCalls } = await runWithWarnSpy(() =>
        runAgentGraph(deps, config, {
          agentsMd: 'AGENTS',
          skills: {},
          createStepMcpClient: buildStepMcpClientFactory(),
          originalPromptText: 'req',
        }),
      )

      expect(Array.of<unknown>(result, attempts, warnCalls)).toEqual([
        {
          status: 'failed',
          message: `フェーズ「Plan」(plan) の実行に失敗しました: agent's structured response did not resolve required for_each field(s) to an array: ${field}`,
          errorKind: 'agent_error',
        },
        3,
        [
          [
            {
              executionStepId: '<execution-step-id>',
              attemptNumber: 1,
              reason: 'missing_required_array_fields',
            },
            'phase "plan" (executionStepId=<execution-step-id>) structured output attempt 1 rejected (missing_required_array_fields), retrying',
          ],
          [
            {
              executionStepId: '<execution-step-id>',
              attemptNumber: 2,
              reason: 'missing_required_array_fields',
            },
            'phase "plan" (executionStepId=<execution-step-id>) structured output attempt 2 rejected (missing_required_array_fields), retrying',
          ],
        ],
      ])
    },
  )

  it('fails immediately, without retrying, when for_each references a phase key that does not exist', async () => {
    let attempts = 0
    const { deps } = buildDeps(() => {
      attempts++
      return Promise.resolve({ structuredResponse: {} })
    })
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'investigate',
          label: 'Investigate',
          model: 'm',
          prompt: 'do investigate',
          forEach: 'missing.items',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
    })

    expect(result).toEqual({
      status: 'failed',
      message:
        'フェーズ「Investigate」(investigate) の実行に失敗しました: for_each の参照先 "missing.items" が配列ではありません',
      errorKind: 'agent_error',
    })
    expect(attempts).toBe(0)
  })

  it('passes an empty for_each result to the following phase without invoking an item', async () => {
    const { deps, calls } = buildDeps((call) =>
      Promise.resolve(
        call.messageText.includes('do plan')
          ? { structuredResponse: { items: [] } }
          : { structuredResponse: { summary: 'no entries' } },
      ),
    )
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'plan',
          label: 'Plan',
          model: 'm',
          prompt: 'do plan',
          subagents: [],
          skills: [],
          tools: [],
          output: { items: { type: 'array' } },
        },
        {
          key: 'investigate',
          label: 'Investigate',
          model: 'm',
          prompt: 'do investigate',
          forEach: 'plan.items',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
        {
          key: 'summary',
          label: 'Summary',
          model: 'fictional-model',
          prompt: 'write summary',
          subagents: [],
          skills: [],
          tools: [],
          output: { summary: { type: 'string' } },
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
    })

    expect(
      Array.of<unknown>(
        result,
        calls.filter((call) => call.messageText.includes('do plan')).length,
        calls.map((call) => call.messageText),
      ),
    ).toEqual([
      {
        status: 'completed',
        message: '3フェーズの実行が完了しました (Plan → Investigate → Summary)',
      },
      1,
      [
        buildPhaseMessageText({
          originalPromptText: 'req',
          phaseKeys: ['plan', 'investigate', 'summary'],
          phaseKey: 'plan',
          phasePrompt: 'do plan',
          item: undefined,
          priorResults: {},
        }),
        buildPhaseMessageText({
          originalPromptText: 'req',
          phaseKeys: ['plan', 'investigate', 'summary'],
          phaseKey: 'summary',
          phasePrompt: 'write summary',
          item: undefined,
          priorResults: {
            plan: { items: [] },
            investigate: [],
          },
        }),
      ],
    ])
  })

  it('notifies onStepsChanged with a running step, then a completed step, for a non-for_each phase', async () => {
    const { deps } = buildDeps(() =>
      Promise.resolve({ structuredResponse: { value: 'A-OUT' } }),
    )
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'stepA',
          label: 'Step A',
          model: 'model-a',
          prompt: 'do A',
          subagents: [],
          skills: [],
          tools: [],
          output: { value: { type: 'string' } },
        },
      ],
    }
    const notifications: (readonly StrategyTaskStep[])[] = []

    await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
      onStepsChanged: (steps) => notifications.push(steps),
    })

    expect(notifications.map(normalizeStepTimestamps)).toEqual([
      [
        {
          phaseKey: 'stepA',
          executionStepId: '<execution-step-id-1>',
          label: 'Step A',
          model: 'model-a',
          status: 'running',
          startedAt: '<started-at>',
          traceId: NOOP_TRACE_ID,
          spanId: NOOP_SPAN_ID,
        },
      ],
      [
        {
          phaseKey: 'stepA',
          executionStepId: '<execution-step-id-1>',
          label: 'Step A',
          model: 'model-a',
          status: 'completed',
          output: { value: 'A-OUT' },
          startedAt: '<started-at>',
          finishedAt: '<finished-at>',
          traceId: NOOP_TRACE_ID,
          spanId: NOOP_SPAN_ID,
        },
      ],
    ])
  })

  it('keeps the model-call limit marker on the completed phase step', async () => {
    const notifications: (readonly StrategyTaskStep[])[] = []
    const deps: RunAgentGraphDeps = {
      createChatModel: () => new FakeChatModel({}),
      buildPhaseAgent: (options): CompiledPhaseAgent => ({
        invoke: () => {
          options.onModelCallLimitReached?.()
          return Promise.resolve({ structuredResponse: { value: 'A-OUT' } })
        },
      }),
    }
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'stepA',
          label: 'Step A',
          model: 'model-a',
          prompt: 'do A',
          subagents: [],
          skills: [],
          tools: [],
          output: { value: { type: 'string' } },
        },
      ],
    }

    await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
      onStepsChanged: (steps) => notifications.push(steps),
    })

    expect(notifications.map(normalizeStepTimestamps)).toEqual([
      [
        {
          phaseKey: 'stepA',
          executionStepId: '<execution-step-id-1>',
          label: 'Step A',
          model: 'model-a',
          status: 'running',
          startedAt: '<started-at>',
          traceId: NOOP_TRACE_ID,
          spanId: NOOP_SPAN_ID,
        },
      ],
      [
        {
          phaseKey: 'stepA',
          executionStepId: '<execution-step-id-1>',
          label: 'Step A',
          model: 'model-a',
          status: 'running',
          modelCallLimitReached: true,
          startedAt: '<started-at>',
          traceId: NOOP_TRACE_ID,
          spanId: NOOP_SPAN_ID,
        },
      ],
      [
        {
          phaseKey: 'stepA',
          executionStepId: '<execution-step-id-1>',
          label: 'Step A',
          model: 'model-a',
          status: 'completed',
          modelCallLimitReached: true,
          output: { value: 'A-OUT' },
          startedAt: '<started-at>',
          finishedAt: '<finished-at>',
          traceId: NOOP_TRACE_ID,
          spanId: NOOP_SPAN_ID,
        },
      ],
    ])
  })

  it('records one step per for_each item, extracting item_label via label_field', async () => {
    const { deps } = buildDeps((call) =>
      Promise.resolve(
        call.messageText.includes('do plan')
          ? {
              structuredResponse: {
                hypotheses: [{ title: 'H1' }, { title: 'H2' }],
              },
            }
          : { structuredResponse: {} },
      ),
    )
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'plan',
          label: 'Plan',
          model: 'm',
          prompt: 'do plan',
          subagents: [],
          skills: [],
          tools: [],
          output: { hypotheses: { type: 'array' } },
        },
        {
          key: 'investigate',
          label: 'Investigate',
          model: 'm',
          prompt: 'do investigate',
          forEach: 'plan.hypotheses',
          labelField: 'title',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }
    const notifications: (readonly StrategyTaskStep[])[] = []

    await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
      onStepsChanged: (steps) => notifications.push(steps),
    })

    const last = notifications.at(-1)
    expect(
      last === undefined ? undefined : normalizeStepTimestamps(last),
    ).toEqual([
      {
        phaseKey: 'plan',
        executionStepId: '<execution-step-id-1>',
        label: 'Plan',
        model: 'm',
        status: 'completed',
        output: { hypotheses: [{ title: 'H1' }, { title: 'H2' }] },
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: NOOP_TRACE_ID,
        spanId: NOOP_SPAN_ID,
      },
      {
        phaseKey: 'investigate',
        executionStepId: '<execution-step-id-2>',
        label: 'Investigate',
        model: 'm',
        status: 'completed',
        item: { title: 'H1' },
        itemLabel: 'H1',
        output: {},
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: NOOP_TRACE_ID,
        spanId: NOOP_SPAN_ID,
      },
      {
        phaseKey: 'investigate',
        executionStepId: '<execution-step-id-3>',
        label: 'Investigate',
        model: 'm',
        status: 'completed',
        item: { title: 'H2' },
        itemLabel: 'H2',
        output: {},
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: NOOP_TRACE_ID,
        spanId: NOOP_SPAN_ID,
      },
    ])
  })

  it('gives each for_each item a distinct executionStepId', async () => {
    const { deps, calls } = buildDeps((call) =>
      Promise.resolve(
        call.messageText.includes('do plan')
          ? { structuredResponse: { items: ['a', 'b'] } }
          : { structuredResponse: {} },
      ),
    )
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'plan',
          label: 'Plan',
          model: 'm',
          prompt: 'do plan',
          subagents: [],
          skills: [],
          tools: [],
          output: { items: { type: 'array', items: { type: 'string' } } },
        },
        {
          key: 'work',
          label: 'Work',
          model: 'm',
          prompt: 'do work',
          forEach: 'plan.items',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
    })

    expect(
      normalizeExecutionStepIds(calls).map((call) => call.executionStepId),
    ).toEqual([
      '<execution-step-id-1>',
      '<execution-step-id-2>',
      '<execution-step-id-3>',
    ])
  })

  it('continues with remaining items and threads only successful outputs into the next phase when some for_each items fail', async () => {
    const messageTextForWorkItem = (item: unknown): string =>
      buildPhaseMessageText({
        originalPromptText: 'req',
        phaseKeys: ['plan', 'work', 'summarize'],
        phaseKey: 'work',
        phasePrompt: 'do work',
        item,
        priorResults: { plan: { items: ['a', 'b'] } },
      })
    const { deps, calls } = buildDeps((call) => {
      if (call.messageText.includes('do plan')) {
        return Promise.resolve({ structuredResponse: { items: ['a', 'b'] } })
      }
      if (call.messageText === messageTextForWorkItem('a')) {
        return Promise.reject(new Error('item a failed'))
      }
      if (call.messageText === messageTextForWorkItem('b')) {
        return Promise.resolve({ structuredResponse: { value: 'b-done' } })
      }
      return Promise.resolve({ structuredResponse: {} })
    })
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'plan',
          label: 'Plan',
          model: 'm',
          prompt: 'do plan',
          subagents: [],
          skills: [],
          tools: [],
          output: { items: { type: 'array', items: { type: 'string' } } },
        },
        {
          key: 'work',
          label: 'Work',
          model: 'm',
          prompt: 'do work',
          forEach: 'plan.items',
          maxParallel: 1,
          subagents: [],
          skills: [],
          tools: [],
          output: { value: { type: 'string' } },
        },
        {
          key: 'summarize',
          label: 'Summarize',
          model: 'm',
          prompt: 'do summarize',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }
    const notifications: (readonly StrategyTaskStep[])[] = []

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
      onStepsChanged: (steps) => notifications.push(steps),
    })

    expect(result).toEqual({
      status: 'completed',
      message: '3フェーズの実行が完了しました (Plan → Work → Summarize)',
    })
    expect(normalizeExecutionStepIds(calls).at(-1)).toEqual({
      systemPrompt: 'AGENTS',
      messageText: buildPhaseMessageText({
        originalPromptText: 'req',
        phaseKeys: ['plan', 'work', 'summarize'],
        phaseKey: 'summarize',
        phasePrompt: 'do summarize',
        item: undefined,
        priorResults: {
          plan: { items: ['a', 'b'] },
          work: [{ value: 'b-done' }],
        },
      }),
      executionStepId: '<execution-step-id-4>',
    })
    const last = notifications.at(-1)
    expect(
      last === undefined ? undefined : normalizeStepTimestamps(last),
    ).toEqual([
      {
        phaseKey: 'plan',
        executionStepId: '<execution-step-id-1>',
        label: 'Plan',
        model: 'm',
        status: 'completed',
        output: { items: ['a', 'b'] },
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: NOOP_TRACE_ID,
        spanId: NOOP_SPAN_ID,
      },
      {
        phaseKey: 'work',
        executionStepId: '<execution-step-id-2>',
        label: 'Work',
        model: 'm',
        status: 'failed',
        item: 'a',
        error: 'item a failed',
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: NOOP_TRACE_ID,
        spanId: NOOP_SPAN_ID,
      },
      {
        phaseKey: 'work',
        executionStepId: '<execution-step-id-3>',
        label: 'Work',
        model: 'm',
        status: 'completed',
        item: 'b',
        output: { value: 'b-done' },
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: NOOP_TRACE_ID,
        spanId: NOOP_SPAN_ID,
      },
      {
        phaseKey: 'summarize',
        executionStepId: '<execution-step-id-4>',
        label: 'Summarize',
        model: 'm',
        status: 'completed',
        output: {},
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: NOOP_TRACE_ID,
        spanId: NOOP_SPAN_ID,
      },
    ])
  })

  it('fails the phase when every for_each item fails', async () => {
    const messageTextForWorkItem = (item: unknown): string =>
      buildPhaseMessageText({
        originalPromptText: 'req',
        phaseKeys: ['plan', 'work'],
        phaseKey: 'work',
        phasePrompt: 'do work',
        item,
        priorResults: { plan: { items: ['x', 'y'] } },
      })
    const { deps } = buildDeps((call) => {
      if (call.messageText.includes('do plan')) {
        return Promise.resolve({ structuredResponse: { items: ['x', 'y'] } })
      }
      return call.messageText === messageTextForWorkItem('x')
        ? Promise.reject(new Error('x failed'))
        : Promise.reject(new Error('y failed'))
    })
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'plan',
          label: 'Plan',
          model: 'm',
          prompt: 'do plan',
          subagents: [],
          skills: [],
          tools: [],
          output: { items: { type: 'array', items: { type: 'string' } } },
        },
        {
          key: 'work',
          label: 'Work',
          model: 'm',
          prompt: 'do work',
          forEach: 'plan.items',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
    })

    expect(result).toEqual({
      status: 'failed',
      message:
        'フェーズ「Work」(work) の実行に失敗しました: for_each の全要素 (2件) が失敗しました: x failed',
      errorKind: 'agent_error',
    })
  })

  it('preserves the usage_limit error classification when every for_each item fails from a usage limit error', async () => {
    const usageLimitError = Object.assign(new Error('usage limit exceeded'), {
      rateLimitType: 'capacity',
    })
    const { deps } = buildDeps((call) =>
      call.messageText.includes('do plan')
        ? Promise.resolve({ structuredResponse: { items: ['a', 'b'] } })
        : Promise.reject(usageLimitError),
    )
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'plan',
          label: 'Plan',
          model: 'm',
          prompt: 'do plan',
          subagents: [],
          skills: [],
          tools: [],
          output: { items: { type: 'array', items: { type: 'string' } } },
        },
        {
          key: 'work',
          label: 'Work',
          model: 'm',
          prompt: 'do work',
          forEach: 'plan.items',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
    })

    expect(result).toEqual({
      status: 'failed',
      message:
        'フェーズ「Work」(work) の実行に失敗しました: usage limit exceeded',
      errorKind: 'usage_limit',
    })
  })

  it('records a failed step with an error and no output when a phase fails', async () => {
    const { deps } = buildDeps(() => Promise.resolve({}))
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'p',
          label: 'P',
          model: 'm',
          prompt: 'do p',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }
    const notifications: (readonly StrategyTaskStep[])[] = []

    await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
      onStepsChanged: (steps) => notifications.push(steps),
    })

    const last = notifications.at(-1)
    expect(
      last === undefined ? undefined : normalizeStepTimestamps(last),
    ).toEqual([
      {
        phaseKey: 'p',
        executionStepId: '<execution-step-id-1>',
        label: 'P',
        model: 'm',
        status: 'failed',
        error: 'agent did not return a structured response',
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: NOOP_TRACE_ID,
        spanId: NOOP_SPAN_ID,
      },
    ])
  })

  it('skips a completed phase on resume, reusing its output for downstream phases', async () => {
    const { deps, calls } = buildDeps(() =>
      Promise.resolve({ structuredResponse: { value: 'B-OUT' } }),
    )
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'stepA',
          label: 'Step A',
          model: 'model-a',
          prompt: 'do A',
          subagents: [],
          skills: [],
          tools: [],
          output: { value: { type: 'string' } },
        },
        {
          key: 'stepB',
          label: 'Step B',
          model: 'model-b',
          prompt: 'do B',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }
    const notifications: (readonly StrategyTaskStep[])[] = []

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
      onStepsChanged: (steps) => notifications.push(steps),
      previousSteps: [
        buildPreviousStep({
          phaseKey: 'stepA',
          label: 'Step A',
          model: 'model-a',
          executionStepId: '11111111-1111-1111-1111-111111111111',
          status: 'completed',
          output: { value: 'PREV-A-OUT' },
        }),
      ],
    })

    expect(result).toEqual({
      status: 'completed',
      message: '2フェーズの実行が完了しました (Step A → Step B)',
    })
    expect(normalizeExecutionStepIds(calls)).toEqual([
      {
        systemPrompt: 'AGENTS',
        messageText: buildPhaseMessageText({
          originalPromptText: 'req',
          phaseKeys: ['stepA', 'stepB'],
          phaseKey: 'stepB',
          phasePrompt: 'do B',
          item: undefined,
          priorResults: { stepA: { value: 'PREV-A-OUT' } },
        }),
        executionStepId: '<execution-step-id-1>',
      },
    ])
    const last = notifications.at(-1)
    expect(
      last === undefined ? undefined : normalizeStepTimestamps(last),
    ).toEqual([
      {
        phaseKey: 'stepA',
        executionStepId: '<execution-step-id-1>',
        label: 'Step A',
        model: 'model-a',
        status: 'completed',
        output: { value: 'PREV-A-OUT' },
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: 'prev-trace',
        spanId: 'prev-span',
      },
      {
        phaseKey: 'stepB',
        executionStepId: '<execution-step-id-2>',
        label: 'Step B',
        model: 'model-b',
        status: 'completed',
        output: { value: 'B-OUT' },
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: NOOP_TRACE_ID,
        spanId: NOOP_SPAN_ID,
      },
    ])
  })

  it('re-runs a non-completed phase on resume, reusing its original executionStepId', async () => {
    const { deps, calls } = buildDeps(() =>
      Promise.resolve({ structuredResponse: { value: 'A-OUT' } }),
    )
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'stepA',
          label: 'Step A',
          model: 'model-a',
          prompt: 'do A',
          subagents: [],
          skills: [],
          tools: [],
          output: { value: { type: 'string' } },
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      createStepMcpClient: buildStepMcpClientFactory(),
      originalPromptText: 'req',
      previousSteps: [
        buildPreviousStep({
          phaseKey: 'stepA',
          label: 'Step A',
          model: 'model-a',
          executionStepId: '22222222-2222-2222-2222-222222222222',
          status: 'failed',
          error: 'boom',
        }),
      ],
    })

    expect(result).toEqual({
      status: 'completed',
      message: '1フェーズの実行が完了しました (Step A)',
    })
    expect(calls).toEqual([
      {
        systemPrompt: 'AGENTS',
        messageText: buildPhaseMessageText({
          originalPromptText: 'req',
          phaseKeys: ['stepA'],
          phaseKey: 'stepA',
          phasePrompt: 'do A',
          item: undefined,
          priorResults: {},
        }),
        executionStepId: '22222222-2222-2222-2222-222222222222',
      },
    ])
  })

  describe('for_each resume', () => {
    const buildForEachConfig = (): AgentGraphConfig => ({
      phases: [
        {
          key: 'plan',
          label: 'Plan',
          model: 'm',
          prompt: 'do plan',
          subagents: [],
          skills: [],
          tools: [],
          output: { hypotheses: { type: 'array' } },
        },
        {
          key: 'investigate',
          label: 'Investigate',
          model: 'm',
          prompt: 'do investigate',
          forEach: 'plan.hypotheses',
          labelField: 'title',
          subagents: [],
          skills: [],
          tools: [],
          output: {},
        },
      ],
    })

    it('skips a completed for_each item on resume, reusing its output', async () => {
      const { deps, calls } = buildDeps((call) =>
        call.messageText.includes('do plan')
          ? Promise.resolve({
              structuredResponse: { hypotheses: [{ title: 'H1' }] },
            })
          : Promise.reject(
              new Error(
                'investigate should not be invoked for a completed item',
              ),
            ),
      )
      const notifications: (readonly StrategyTaskStep[])[] = []

      const result = await runAgentGraph(deps, buildForEachConfig(), {
        agentsMd: 'AGENTS',
        skills: {},
        createStepMcpClient: buildStepMcpClientFactory(),
        originalPromptText: 'req',
        onStepsChanged: (steps) => notifications.push(steps),
        previousSteps: [
          buildPreviousStep({
            executionStepId: '33333333-3333-3333-3333-333333333333',
            status: 'completed',
            item: { title: 'H1' },
            itemLabel: 'H1',
            output: { note: 'H1-OLD' },
          }),
        ],
      })

      expect(result).toEqual({
        status: 'completed',
        message: '2フェーズの実行が完了しました (Plan → Investigate)',
      })
      expect(normalizeExecutionStepIds(calls)).toEqual([
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'investigate'],
            phaseKey: 'plan',
            phasePrompt: 'do plan',
            item: undefined,
            priorResults: {},
          }),
          executionStepId: '<execution-step-id-1>',
        },
      ])
      const last = notifications.at(-1)
      expect(
        last === undefined ? undefined : normalizeStepTimestamps(last),
      ).toEqual([
        {
          phaseKey: 'plan',
          executionStepId: '<execution-step-id-1>',
          label: 'Plan',
          model: 'm',
          status: 'completed',
          output: { hypotheses: [{ title: 'H1' }] },
          startedAt: '<started-at>',
          finishedAt: '<finished-at>',
          traceId: NOOP_TRACE_ID,
          spanId: NOOP_SPAN_ID,
        },
        {
          phaseKey: 'investigate',
          executionStepId: '<execution-step-id-2>',
          label: 'Investigate',
          model: 'm',
          status: 'completed',
          item: { title: 'H1' },
          itemLabel: 'H1',
          output: { note: 'H1-OLD' },
          startedAt: '<started-at>',
          finishedAt: '<finished-at>',
          traceId: 'prev-trace',
          spanId: 'prev-span',
        },
      ])
    })

    it('re-runs a failed for_each item on resume, reusing its executionStepId', async () => {
      const { deps, calls } = buildDeps((call) =>
        call.messageText.includes('do plan')
          ? Promise.resolve({
              structuredResponse: { hypotheses: [{ title: 'H2' }] },
            })
          : Promise.resolve({ structuredResponse: { note: 'H2-NEW' } }),
      )

      const result = await runAgentGraph(deps, buildForEachConfig(), {
        agentsMd: 'AGENTS',
        skills: {},
        createStepMcpClient: buildStepMcpClientFactory(),
        originalPromptText: 'req',
        previousSteps: [
          buildPreviousStep({
            executionStepId: '44444444-4444-4444-4444-444444444444',
            status: 'failed',
            item: { title: 'H2' },
            itemLabel: 'H2',
            error: 'boom',
          }),
        ],
      })

      expect(result).toEqual({
        status: 'completed',
        message: '2フェーズの実行が完了しました (Plan → Investigate)',
      })
      // plan の executionStepId は crypto.randomUUID() による毎回異なる値のため、
      // 実測値を正規化してから比較する。
      const planExecutionStepId = calls[0]?.executionStepId
      expect(planExecutionStepId).toMatch(UUID_PATTERN)
      expect(calls).toEqual([
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'investigate'],
            phaseKey: 'plan',
            phasePrompt: 'do plan',
            item: undefined,
            priorResults: {},
          }),
          executionStepId: planExecutionStepId,
        },
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'investigate'],
            phaseKey: 'investigate',
            phasePrompt: 'do investigate',
            item: { title: 'H2' },
            priorResults: { plan: { hypotheses: [{ title: 'H2' }] } },
          }),
          executionStepId: '44444444-4444-4444-4444-444444444444',
        },
      ])
    })

    it('runs a for_each item with no previous step fresh on resume', async () => {
      const { deps, calls } = buildDeps((call) =>
        call.messageText.includes('do plan')
          ? Promise.resolve({
              structuredResponse: { hypotheses: [{ title: 'H3' }] },
            })
          : Promise.resolve({ structuredResponse: { note: 'H3-NEW' } }),
      )

      const result = await runAgentGraph(deps, buildForEachConfig(), {
        agentsMd: 'AGENTS',
        skills: {},
        createStepMcpClient: buildStepMcpClientFactory(),
        originalPromptText: 'req',
        // previousSteps 自体は存在するが、対象の item (H3) 分は含まれない
        // ケース: 未消費のまま残っている他要素分と誤ってマッチしないことを確認する。
        previousSteps: [
          buildPreviousStep({
            executionStepId: '33333333-3333-3333-3333-333333333333',
            status: 'completed',
            item: { title: 'H1' },
            itemLabel: 'H1',
            output: { note: 'H1-OLD' },
          }),
        ],
      })

      expect(result).toEqual({
        status: 'completed',
        message: '2フェーズの実行が完了しました (Plan → Investigate)',
      })
      expect(normalizeExecutionStepIds(calls)).toEqual([
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'investigate'],
            phaseKey: 'plan',
            phasePrompt: 'do plan',
            item: undefined,
            priorResults: {},
          }),
          executionStepId: '<execution-step-id-1>',
        },
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'investigate'],
            phaseKey: 'investigate',
            phasePrompt: 'do investigate',
            item: { title: 'H3' },
            priorResults: { plan: { hypotheses: [{ title: 'H3' }] } },
          }),
          executionStepId: '<execution-step-id-2>',
        },
      ])
    })
  })

  // for_each は 1 件でも成功すればフェーズ成功として返すため、部分失敗したタスクは
  // 失敗した要素を抱えたまま後続フェーズまで completed で終わっている。
  describe('resume after a partial for_each failure', () => {
    const PLAN_ID = '11111111-1111-1111-1111-111111111111'
    const H1_ID = '22222222-2222-2222-2222-222222222222'
    const H2_ID = '33333333-3333-3333-3333-333333333333'
    const DOWNSTREAM_ID = '44444444-4444-4444-4444-444444444444'

    const buildPhases = (): AgentGraphConfig['phases'] => [
      {
        key: 'plan',
        label: 'Plan',
        model: 'm',
        prompt: 'do plan',
        subagents: [],
        skills: [],
        tools: [],
        output: { hypotheses: { type: 'array' } },
      },
      {
        key: 'investigate',
        label: 'Investigate',
        model: 'm',
        prompt: 'do investigate',
        forEach: 'plan.hypotheses',
        labelField: 'title',
        subagents: [],
        skills: [],
        tools: [],
        output: {},
      },
    ]

    const planStep = buildPreviousStep({
      phaseKey: 'plan',
      label: 'Plan',
      executionStepId: PLAN_ID,
      status: 'completed',
      output: { hypotheses: [{ title: 'H1' }, { title: 'H2' }] },
    })
    const h1Step = buildPreviousStep({
      executionStepId: H1_ID,
      status: 'completed',
      item: { title: 'H1' },
      itemLabel: 'H1',
      output: { note: 'H1-OLD' },
    })
    const h2FailedStep = buildPreviousStep({
      executionStepId: H2_ID,
      status: 'failed',
      item: { title: 'H2' },
      itemLabel: 'H2',
      error: 'boom',
    })

    it('re-runs only the failed item, then re-runs the downstream phase with its original executionStepId', async () => {
      const { deps, calls } = buildDeps((call) =>
        Promise.resolve({
          structuredResponse: call.messageText.includes('do investigate')
            ? { note: 'H2-NEW' }
            : { summary: 'SUMMARY-NEW' },
        }),
      )
      const notifications: (readonly StrategyTaskStep[])[] = []

      const result = await runAgentGraph(
        deps,
        {
          phases: [
            ...buildPhases(),
            {
              key: 'summarize',
              label: 'Summarize',
              model: 'm',
              prompt: 'do summarize',
              subagents: [],
              skills: [],
              tools: [],
              output: {},
            },
          ],
        },
        {
          agentsMd: 'AGENTS',
          skills: {},
          createStepMcpClient: buildStepMcpClientFactory(),
          originalPromptText: 'req',
          onStepsChanged: (steps) => notifications.push(steps),
          previousSteps: [
            planStep,
            h1Step,
            h2FailedStep,
            buildPreviousStep({
              phaseKey: 'summarize',
              label: 'Summarize',
              executionStepId: DOWNSTREAM_ID,
              status: 'completed',
              output: { summary: 'SUMMARY-OLD' },
            }),
          ],
        },
      )

      expect(result).toEqual({
        status: 'completed',
        message:
          '3フェーズの実行が完了しました (Plan → Investigate → Summarize)',
      })
      // plan と H1 は完了済みなので呼ばれず、H2 のやり直し結果 (H2-NEW) が
      // 完了済みだった summarize にも渡る。
      expect(calls).toEqual([
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'investigate', 'summarize'],
            phaseKey: 'investigate',
            phasePrompt: 'do investigate',
            item: { title: 'H2' },
            priorResults: {
              plan: { hypotheses: [{ title: 'H1' }, { title: 'H2' }] },
            },
          }),
          executionStepId: H2_ID,
        },
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'investigate', 'summarize'],
            phaseKey: 'summarize',
            phasePrompt: 'do summarize',
            item: undefined,
            priorResults: {
              plan: { hypotheses: [{ title: 'H1' }, { title: 'H2' }] },
              investigate: [{ note: 'H1-OLD' }, { note: 'H2-NEW' }],
            },
          }),
          executionStepId: DOWNSTREAM_ID,
        },
      ])
      const last = notifications.at(-1)
      const timestamps = {
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
      }
      expect(
        last === undefined ? undefined : normalizeStepTimestamps(last),
      ).toEqual([
        // スキップした plan / H1 は前回のステップがそのまま残る。
        {
          ...planStep,
          ...timestamps,
          executionStepId: '<execution-step-id-1>',
        },
        {
          ...h1Step,
          ...timestamps,
          executionStepId: '<execution-step-id-2>',
        },
        // やり直した H2 と summarize は新しい出力で completed になる。
        {
          phaseKey: 'investigate',
          executionStepId: '<execution-step-id-3>',
          label: 'Investigate',
          model: 'm',
          status: 'completed',
          item: { title: 'H2' },
          itemLabel: 'H2',
          output: { note: 'H2-NEW' },
          ...timestamps,
          traceId: NOOP_TRACE_ID,
          spanId: NOOP_SPAN_ID,
        },
        {
          phaseKey: 'summarize',
          executionStepId: '<execution-step-id-4>',
          label: 'Summarize',
          model: 'm',
          status: 'completed',
          output: { summary: 'SUMMARY-NEW' },
          ...timestamps,
          traceId: NOOP_TRACE_ID,
          spanId: NOOP_SPAN_ID,
        },
      ])
    })

    it('re-runs a downstream for_each phase for every item, reusing existing executionStepIds and giving the recovered item a fresh one', async () => {
      const { deps, calls } = buildDeps((call) =>
        Promise.resolve({
          structuredResponse: call.messageText.includes('do investigate')
            ? { note: 'H2-NEW' }
            : { verdict: 'REVIEW-NEW' },
        }),
      )

      const result = await runAgentGraph(
        deps,
        {
          phases: [
            ...buildPhases(),
            {
              key: 'review',
              label: 'Review',
              model: 'm',
              prompt: 'do review',
              forEach: 'plan.hypotheses',
              labelField: 'title',
              subagents: [],
              skills: [],
              tools: [],
              output: {},
            },
          ],
        },
        {
          agentsMd: 'AGENTS',
          skills: {},
          createStepMcpClient: buildStepMcpClientFactory(),
          originalPromptText: 'req',
          previousSteps: [
            planStep,
            h1Step,
            h2FailedStep,
            // review は H1 の分だけ完了していた (H2 は investigate が失敗したため未着手)。
            buildPreviousStep({
              phaseKey: 'review',
              label: 'Review',
              executionStepId: DOWNSTREAM_ID,
              status: 'completed',
              item: { title: 'H1' },
              itemLabel: 'H1',
              output: { verdict: 'REVIEW-OLD' },
            }),
          ],
        },
      )

      expect(result).toEqual({
        status: 'completed',
        message: '3フェーズの実行が完了しました (Plan → Investigate → Review)',
      })
      const priorResults = {
        plan: { hypotheses: [{ title: 'H1' }, { title: 'H2' }] },
        investigate: [{ note: 'H1-OLD' }, { note: 'H2-NEW' }],
      }
      // 復活した H2 の review は前回の step が無いので新規採番される。実行ごとに
      // 変わる値のため、実測値が UUID で、既存のどの step の id とも衝突しない
      // (= 別ステップとして write_note の upsert キーが分かれる) ことを確認した上で
      // 期待値に使う。
      const freshReviewId = calls[2]?.executionStepId
      expect(freshReviewId).toMatch(UUID_PATTERN)
      expect([PLAN_ID, H1_ID, H2_ID, DOWNSTREAM_ID]).not.toContain(
        freshReviewId,
      )
      expect(calls).toEqual([
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'investigate', 'review'],
            phaseKey: 'investigate',
            phasePrompt: 'do investigate',
            item: { title: 'H2' },
            priorResults: {
              plan: { hypotheses: [{ title: 'H1' }, { title: 'H2' }] },
            },
          }),
          executionStepId: H2_ID,
        },
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'investigate', 'review'],
            phaseKey: 'review',
            phasePrompt: 'do review',
            item: { title: 'H1' },
            priorResults,
          }),
          executionStepId: DOWNSTREAM_ID,
        },
        {
          systemPrompt: 'AGENTS',
          messageText: buildPhaseMessageText({
            originalPromptText: 'req',
            phaseKeys: ['plan', 'investigate', 'review'],
            phaseKey: 'review',
            phasePrompt: 'do review',
            item: { title: 'H2' },
            priorResults,
          }),
          executionStepId: freshReviewId,
        },
      ])
    })
  })
})

describe('buildPhaseMessageText', () => {
  it('joins the original prompt, phase prompt, item, and prior results', () => {
    expect(
      buildPhaseMessageText({
        originalPromptText: 'original request',
        phaseKeys: ['phase-a', 'phase-b', 'phase-c'],
        phaseKey: 'phase-b',
        phasePrompt: 'do the thing',
        item: { title: 'hypothesis 1' },
        priorResults: { plan: { hypotheses: [{ title: 'hypothesis 1' }] } },
      }),
    ).toBe(
      [
        'original request',
        'この巡回は phase-a → phase-b → phase-c の順に進む。あなたは phase-b を担当する。依頼のうち他の phase の担当分は、後の phase が行う',
        'do the thing',
        '割り当てられた対象:\n```json\n{\n  "title": "hypothesis 1"\n}\n```',
        'これまでのフェーズの結果:\n```json\n{\n  "plan": {\n    "hypotheses": [\n      {\n        "title": "hypothesis 1"\n      }\n    ]\n  }\n}\n```',
      ].join('\n\n---\n\n'),
    )
  })

  it.each([
    {
      name: 'the first phase',
      phaseKeys: ['phase-a', 'phase-b'],
      phaseKey: 'phase-a',
      scopeMessage:
        'この巡回は phase-a → phase-b の順に進む。あなたは phase-a を担当する。依頼のうち他の phase の担当分は、後の phase が行う',
    },
    {
      name: 'a middle phase',
      phaseKeys: ['phase-a', 'phase-b', 'phase-c'],
      phaseKey: 'phase-b',
      scopeMessage:
        'この巡回は phase-a → phase-b → phase-c の順に進む。あなたは phase-b を担当する。依頼のうち他の phase の担当分は、後の phase が行う',
    },
    {
      name: 'the last phase',
      phaseKeys: ['phase-a', 'phase-b'],
      phaseKey: 'phase-b',
      scopeMessage:
        'この巡回は phase-a → phase-b の順に進む。あなたは phase-b を担当する。依頼のうち他の phase の担当分は、後の phase が行う',
    },
    {
      name: 'a single phase',
      phaseKeys: ['phase-a'],
      phaseKey: 'phase-a',
      scopeMessage:
        'この巡回は phase-a の順に進む。あなたは phase-a を担当する。依頼のうち他の phase の担当分は、後の phase が行う',
    },
  ])(
    'describes responsibility for $name',
    ({ phaseKeys, phaseKey, scopeMessage }) => {
      expect(
        buildPhaseMessageText({
          originalPromptText: 'original request',
          phaseKeys,
          phaseKey,
          phasePrompt: 'do the thing',
          item: undefined,
          priorResults: {},
        }),
      ).toBe(
        ['original request', scopeMessage, 'do the thing'].join('\n\n---\n\n'),
      )
    },
  )
})
