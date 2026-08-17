import { describe, expect, it } from 'vitest'

import {
  computeElapsed,
  formatElapsed,
  sourceLabel,
} from '#components/strategy-shell/task-run-view'

describe('sourceLabel', () => {
  it.each([
    ['frontend', 'フローティングチャット'],
    ['unknown-source', 'unknown-source'],
  ])('%s を %s に変換する', (source, expected) => {
    expect(sourceLabel(source)).toBe(expected)
  })
})

describe('formatElapsed', () => {
  it.each([
    [59000, '59s'],
    [65000, '1m5s'],
    [0, '0s'],
  ])('%dms を %s に変換する', (ms, expected) => {
    expect(formatElapsed(ms)).toBe(expected)
  })
})

describe('computeElapsed', () => {
  it('completed タスクは createdAt から updatedAt までの経過時間を返す', () => {
    expect(
      computeElapsed({
        taskId: 't1',
        prompt: 'p',
        source: 'frontend',
        phase: 'completed',
        createdAt: '2026-08-15T00:00:00.000Z',
        updatedAt: '2026-08-15T00:01:05.000Z',
        errorSummary: null,
      }),
    ).toBe('1m5s')
  })

  it('failed タスクは createdAt から updatedAt までの経過時間を返す', () => {
    expect(
      computeElapsed({
        taskId: 't1',
        prompt: 'p',
        source: 'frontend',
        phase: 'failed',
        createdAt: '2026-08-15T00:00:00.000Z',
        updatedAt: '2026-08-15T00:00:59.000Z',
        errorSummary: 'boom',
      }),
    ).toBe('59s')
  })

  it('running タスクは現在時刻までの経過時間を返す', () => {
    const result = computeElapsed({
      taskId: 't1',
      prompt: 'p',
      source: 'frontend',
      phase: 'running',
      createdAt: new Date(Date.now() - 5000).toISOString(),
      updatedAt: new Date(Date.now() - 5000).toISOString(),
      errorSummary: null,
    })
    expect(result).toMatch(/^[45]s$/)
  })
})
