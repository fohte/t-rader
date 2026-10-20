import { describe, expect, it } from 'vitest'

import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import { toStepJson } from '#strategy-agent/agent-graph/step'

describe('toStepJson', () => {
  it('converts a running for_each step to snake_case, keeping optional fields present', () => {
    const step: StrategyTaskStep = {
      phaseKey: 'stage_b',
      executionStepId: 'exec-1',
      label: '項目の処理',
      model: 'demo-model-a',
      status: 'running',
      item: { label: 'item-a' },
      itemLabel: 'item-a',
      startedAt: '2026-01-01T00:00:00.000Z',
      traceId: 'trace-1',
      spanId: 'span-1',
    }

    expect(toStepJson(step)).toEqual({
      phase_key: 'stage_b',
      execution_step_id: 'exec-1',
      label: '項目の処理',
      model: 'demo-model-a',
      status: 'running',
      item: { label: 'item-a' },
      item_label: 'item-a',
      started_at: '2026-01-01T00:00:00.000Z',
      trace_id: 'trace-1',
      span_id: 'span-1',
    })
  })

  it('keeps output/finished_at and omits item/item_label/error when unset (non-for_each, completed step)', () => {
    const step: StrategyTaskStep = {
      phaseKey: 'stage_a',
      executionStepId: 'exec-2',
      label: '項目の作成',
      model: 'demo-model-a',
      status: 'completed',
      output: { entries: [] },
      startedAt: '2026-01-01T00:00:00.000Z',
      finishedAt: '2026-01-01T00:00:05.000Z',
      traceId: 'trace-2',
      spanId: 'span-2',
    }

    expect(toStepJson(step)).toEqual({
      phase_key: 'stage_a',
      execution_step_id: 'exec-2',
      label: '項目の作成',
      model: 'demo-model-a',
      status: 'completed',
      output: { entries: [] },
      started_at: '2026-01-01T00:00:00.000Z',
      finished_at: '2026-01-01T00:00:05.000Z',
      trace_id: 'trace-2',
      span_id: 'span-2',
    })
  })

  it('keeps item but omits item_label when label_field is not configured', () => {
    const step: StrategyTaskStep = {
      phaseKey: 'stage_b',
      executionStepId: 'exec-4',
      label: '項目の処理',
      model: 'demo-model-a',
      status: 'running',
      item: { label: 'item-a' },
      startedAt: '2026-01-01T00:00:00.000Z',
      traceId: 'trace-4',
      spanId: 'span-4',
    }

    expect(toStepJson(step)).toEqual({
      phase_key: 'stage_b',
      execution_step_id: 'exec-4',
      label: '項目の処理',
      model: 'demo-model-a',
      status: 'running',
      item: { label: 'item-a' },
      started_at: '2026-01-01T00:00:00.000Z',
      trace_id: 'trace-4',
      span_id: 'span-4',
    })
  })

  it('includes error and omits output for a failed step', () => {
    const step: StrategyTaskStep = {
      phaseKey: 'stage_a',
      executionStepId: 'exec-3',
      label: '項目の作成',
      model: 'demo-model-a',
      status: 'failed',
      error: 'agent did not return a structured response',
      startedAt: '2026-01-01T00:00:00.000Z',
      finishedAt: '2026-01-01T00:00:05.000Z',
      traceId: 'trace-3',
      spanId: 'span-3',
    }

    expect(toStepJson(step)).toEqual({
      phase_key: 'stage_a',
      execution_step_id: 'exec-3',
      label: '項目の作成',
      model: 'demo-model-a',
      status: 'failed',
      started_at: '2026-01-01T00:00:00.000Z',
      finished_at: '2026-01-01T00:00:05.000Z',
      trace_id: 'trace-3',
      span_id: 'span-3',
      error: 'agent did not return a structured response',
    })
  })
})
