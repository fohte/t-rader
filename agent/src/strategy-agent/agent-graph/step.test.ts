import { describe, expect, it } from 'vitest'

import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import { toStepJson } from '#strategy-agent/agent-graph/step'

describe('toStepJson', () => {
  it('converts a running for_each step to snake_case, keeping optional fields present', () => {
    const step: StrategyTaskStep = {
      phaseKey: 'investigate',
      label: '仮説の調査',
      model: 'deepseek-v4-flash',
      status: 'running',
      item: { title: 'hypothesis 1' },
      itemLabel: 'hypothesis 1',
      startedAt: '2026-01-01T00:00:00.000Z',
      traceId: 'trace-1',
      spanId: 'span-1',
    }

    expect(toStepJson(step)).toEqual({
      phase_key: 'investigate',
      label: '仮説の調査',
      model: 'deepseek-v4-flash',
      status: 'running',
      item: { title: 'hypothesis 1' },
      item_label: 'hypothesis 1',
      started_at: '2026-01-01T00:00:00.000Z',
      trace_id: 'trace-1',
      span_id: 'span-1',
    })
  })

  it('keeps output/finished_at and omits item/item_label/error when unset (non-for_each, completed step)', () => {
    const step: StrategyTaskStep = {
      phaseKey: 'plan',
      label: '調査計画',
      model: 'claude-opus-4',
      status: 'completed',
      output: { hypotheses: [] },
      startedAt: '2026-01-01T00:00:00.000Z',
      finishedAt: '2026-01-01T00:00:05.000Z',
      traceId: 'trace-2',
      spanId: 'span-2',
    }

    expect(toStepJson(step)).toEqual({
      phase_key: 'plan',
      label: '調査計画',
      model: 'claude-opus-4',
      status: 'completed',
      output: { hypotheses: [] },
      started_at: '2026-01-01T00:00:00.000Z',
      finished_at: '2026-01-01T00:00:05.000Z',
      trace_id: 'trace-2',
      span_id: 'span-2',
    })
  })

  it('keeps item but omits item_label when label_field is not configured', () => {
    const step: StrategyTaskStep = {
      phaseKey: 'investigate',
      label: '仮説の調査',
      model: 'deepseek-v4-flash',
      status: 'running',
      item: { title: 'hypothesis 1' },
      startedAt: '2026-01-01T00:00:00.000Z',
      traceId: 'trace-4',
      spanId: 'span-4',
    }

    expect(toStepJson(step)).toEqual({
      phase_key: 'investigate',
      label: '仮説の調査',
      model: 'deepseek-v4-flash',
      status: 'running',
      item: { title: 'hypothesis 1' },
      started_at: '2026-01-01T00:00:00.000Z',
      trace_id: 'trace-4',
      span_id: 'span-4',
    })
  })

  it('includes error and omits output for a failed step', () => {
    const step: StrategyTaskStep = {
      phaseKey: 'plan',
      label: '調査計画',
      model: 'claude-opus-4',
      status: 'failed',
      error: 'agent did not return a structured response',
      startedAt: '2026-01-01T00:00:00.000Z',
      finishedAt: '2026-01-01T00:00:05.000Z',
      traceId: 'trace-3',
      spanId: 'span-3',
    }

    expect(toStepJson(step)).toEqual({
      phase_key: 'plan',
      label: '調査計画',
      model: 'claude-opus-4',
      status: 'failed',
      started_at: '2026-01-01T00:00:00.000Z',
      finished_at: '2026-01-01T00:00:05.000Z',
      trace_id: 'trace-3',
      span_id: 'span-3',
      error: 'agent did not return a structured response',
    })
  })
})
