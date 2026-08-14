import { BaseChatModel } from '@langchain/core/language_models/chat_models'
import type { ChatResult } from '@langchain/core/outputs'
import { DynamicStructuredTool } from '@langchain/core/tools'
import { describe, expect, it } from 'vitest'
import { z } from 'zod'

import type {
  BuildPhaseAgentOptions,
  CompiledPhaseAgent,
  RunAgentGraphDeps,
} from '#strategy-agent/agent-graph/run-agent-graph'
import {
  buildPhaseMessageText,
  runAgentGraph,
} from '#strategy-agent/agent-graph/run-agent-graph'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import type { AgentGraphConfig } from '#strategy-agent/agent-graph/types'

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

interface InvokeCall {
  readonly systemPrompt: string
  readonly messageText: string
}

const buildDeps = (
  invokeImpl: (
    call: InvokeCall,
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
        }
        calls.push(call)
        return invokeImpl(call)
      },
    }),
  }
  return { deps, calls }
}

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
          skills: [],
          tools: [],
          output: { value: { type: 'string' } },
        },
        {
          key: 'stepB',
          label: 'Step B',
          model: 'model-b',
          prompt: 'do B',
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      tools: [],
      originalPromptText: 'original request',
    })

    expect(result).toEqual({
      status: 'completed',
      message: '2フェーズの実行が完了しました (Step A → Step B)',
    })
    expect(calls).toEqual([
      {
        systemPrompt: 'AGENTS',
        messageText: buildPhaseMessageText({
          originalPromptText: 'original request',
          phasePrompt: 'do A',
          item: undefined,
          priorResults: {},
        }),
      },
      {
        systemPrompt: 'AGENTS',
        messageText: buildPhaseMessageText({
          originalPromptText: 'original request',
          phasePrompt: 'do B',
          item: undefined,
          priorResults: { stepA: { value: 'A-OUT' } },
        }),
      },
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
          skills: ['skill-a'],
          tools: ['tool_a'],
          output: {},
        },
      ],
    }

    await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: { 'skill-a': 'body a', 'skill-b': 'body b' },
      tools: [toolA, toolB],
      originalPromptText: 'req',
    })

    expect(capturedOptions).toEqual({
      model: chatModelReturnValue,
      tools: [toolA],
      systemPrompt: 'AGENTS\n\n# Skill: skill-a\n\nbody a',
      responseSchema: { type: 'object', properties: {} },
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
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      tools: [],
      originalPromptText: 'req',
    })

    expect(result).toEqual({
      status: 'completed',
      message: '2フェーズの実行が完了しました (Plan → Work)',
    })
    expect(maxActive).toBe(2)
  })

  it('retries a phase up to 2 times after a missing structured response, then succeeds', async () => {
    let attempts = 0
    const { deps } = buildDeps(() => {
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
          skills: [],
          tools: [],
          output: { ok: { type: 'boolean' } },
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      tools: [],
      originalPromptText: 'req',
    })

    expect(result).toEqual({
      status: 'completed',
      message: '1フェーズの実行が完了しました (P)',
    })
    expect(attempts).toBe(3)
  })

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
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      tools: [],
      originalPromptText: 'req',
    })

    expect(result).toEqual({
      status: 'failed',
      message:
        'フェーズ「P」(p) の実行に失敗しました: agent did not return a structured response',
      errorKind: 'agent_error',
    })
    expect(attempts).toBe(3)
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
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      tools: [],
      originalPromptText: 'req',
    })

    expect(result).toEqual({
      status: 'failed',
      message: 'フェーズ「P」(p) の実行に失敗しました: usage limit exceeded',
      errorKind: 'agent_error',
    })
    expect(attempts).toBe(1)
  })

  it('fails when for_each references a field that is not an array', async () => {
    const { deps } = buildDeps(() =>
      Promise.resolve({ structuredResponse: { value: 'not-an-array' } }),
    )
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'plan',
          label: 'Plan',
          model: 'm',
          prompt: 'do plan',
          skills: [],
          tools: [],
          output: { value: { type: 'string' } },
        },
        {
          key: 'work',
          label: 'Work',
          model: 'm',
          prompt: 'do work',
          forEach: 'plan.value',
          skills: [],
          tools: [],
          output: {},
        },
      ],
    }

    const result = await runAgentGraph(deps, config, {
      agentsMd: 'AGENTS',
      skills: {},
      tools: [],
      originalPromptText: 'req',
    })

    expect(result).toEqual({
      status: 'failed',
      message:
        'フェーズ「Work」(work) の実行に失敗しました: for_each の参照先 "plan.value" が配列ではありません',
      errorKind: 'agent_error',
    })
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
      tools: [],
      originalPromptText: 'req',
      onStepsChanged: (steps) => notifications.push(steps),
    })

    expect(notifications.map(normalizeStepTimestamps)).toEqual([
      [
        {
          phaseKey: 'stepA',
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
      tools: [],
      originalPromptText: 'req',
      onStepsChanged: (steps) => notifications.push(steps),
    })

    const last = notifications.at(-1)
    expect(
      last === undefined ? undefined : normalizeStepTimestamps(last),
    ).toEqual([
      {
        phaseKey: 'plan',
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

  it('records a failed step with an error and no output when a phase fails', async () => {
    const { deps } = buildDeps(() => Promise.resolve({}))
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'p',
          label: 'P',
          model: 'm',
          prompt: 'do p',
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
      tools: [],
      originalPromptText: 'req',
      onStepsChanged: (steps) => notifications.push(steps),
    })

    const last = notifications.at(-1)
    expect(
      last === undefined ? undefined : normalizeStepTimestamps(last),
    ).toEqual([
      {
        phaseKey: 'p',
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
})

describe('buildPhaseMessageText', () => {
  it('joins the original prompt, phase prompt, item, and prior results', () => {
    expect(
      buildPhaseMessageText({
        originalPromptText: 'original request',
        phasePrompt: 'do the thing',
        item: { title: 'hypothesis 1' },
        priorResults: { plan: { hypotheses: [{ title: 'hypothesis 1' }] } },
      }),
    ).toBe(
      [
        'original request',
        'do the thing',
        '割り当てられた対象:\n```json\n{\n  "title": "hypothesis 1"\n}\n```',
        'これまでのフェーズの結果:\n```json\n{\n  "plan": {\n    "hypotheses": [\n      {\n        "title": "hypothesis 1"\n      }\n    ]\n  }\n}\n```',
      ].join('\n\n---\n\n'),
    )
  })

  it('omits the item and prior-results sections when absent', () => {
    expect(
      buildPhaseMessageText({
        originalPromptText: 'original request',
        phasePrompt: 'do the thing',
        item: undefined,
        priorResults: {},
      }),
    ).toBe('original request\n\n---\n\ndo the thing')
  })
})
