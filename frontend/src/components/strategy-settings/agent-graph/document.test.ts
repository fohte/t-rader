import { describe, expect, it } from 'vitest'

import {
  addPhase,
  movePhase,
  parseAgentGraphPhases,
  removePhase,
  setPhaseField,
} from '#components/strategy-settings/agent-graph/document'

const SAMPLE = `phases:
  - key: plan
    label: 調査計画
    model: claude-opus-4
    runs: once
    prompt: 仮説を立てよ
  - key: investigate
    label: 仮説の調査
    model: deepseek-v4-flash
    for_each: plan.hypotheses
    label_field: title
    max_parallel: 4
    prompt: 割り当てられた仮説を検証せよ
    tools: [query_data, write_note]
`

const INCOMPLETE_PHASE = `phases:
  - key: plan
    label: l
`

describe('parseAgentGraphPhases', () => {
  it('空文字列は空配列を返す', () => {
    expect(parseAgentGraphPhases('')).toEqual([])
    expect(parseAgentGraphPhases('   \n')).toEqual([])
  })

  it('runs を含む有効な YAML から編集用フェーズを取り出す (runs 自体はフォームの型に出さない)', () => {
    expect(parseAgentGraphPhases(SAMPLE)).toEqual([
      {
        key: 'plan',
        label: '調査計画',
        model: 'claude-opus-4',
        prompt: '仮説を立てよ',
        forEach: undefined,
        labelField: undefined,
        maxParallel: undefined,
        skills: [],
        tools: [],
        output: {},
      },
      {
        key: 'investigate',
        label: '仮説の調査',
        model: 'deepseek-v4-flash',
        prompt: '割り当てられた仮説を検証せよ',
        forEach: 'plan.hypotheses',
        labelField: 'title',
        maxParallel: 4,
        skills: [],
        tools: ['query_data', 'write_note'],
        output: {},
      },
    ])
  })

  it('構文が壊れた YAML は null を返す', () => {
    expect(parseAgentGraphPhases('phases: [')).toBeNull()
  })

  it('phases がトップレベルに無い YAML は null を返す', () => {
    expect(parseAgentGraphPhases('foo: bar')).toBeNull()
  })

  it('phases が配列でない場合は null を返す', () => {
    expect(parseAgentGraphPhases('phases: not-an-array')).toBeNull()
  })

  it('フェーズが key/label/model/prompt を string で持たない場合は null を返す', () => {
    expect(parseAgentGraphPhases(INCOMPLETE_PHASE)).toBeNull()
  })

  it('phases: [] は空配列として扱う (トグル on だが 0 フェーズの状態)', () => {
    expect(parseAgentGraphPhases('phases: []')).toEqual([])
  })
})

describe('setPhaseField', () => {
  it('指定したフェーズのフィールドだけを書き換え、他はそのまま残す', () => {
    const next = setPhaseField(SAMPLE, 0, 'model', 'claude-sonnet-4')
    expect(parseAgentGraphPhases(next)).toEqual([
      expect.objectContaining({ key: 'plan', model: 'claude-sonnet-4' }),
      expect.objectContaining({ key: 'investigate' }),
    ])
    // runs は素通しされたままであること
    expect(next).toContain('runs: once')
  })

  it('改行を含む prompt を block literal として書き込む', () => {
    const next = setPhaseField(SAMPLE, 0, 'prompt', 'line1\nline2')
    expect(next).toContain('prompt: |-')
    expect(parseAgentGraphPhases(next)?.[0]?.prompt).toBe('line1\nline2')
  })
})

describe('addPhase', () => {
  it('末尾に空のフェーズを追加する', () => {
    const next = addPhase(SAMPLE)
    const phases = parseAgentGraphPhases(next)
    expect(phases).toHaveLength(3)
    expect(phases?.[2]).toEqual({
      key: 'phase-3',
      label: '新しいフェーズ',
      model: '',
      prompt: '',
      forEach: undefined,
      labelField: undefined,
      maxParallel: undefined,
      skills: [],
      tools: [],
      output: {},
    })
  })

  it('空文字列からフェーズを追加すると 1 件になる', () => {
    const next = addPhase('')
    expect(parseAgentGraphPhases(next)).toHaveLength(1)
  })
})

describe('removePhase', () => {
  it('指定 index のフェーズを取り除く', () => {
    const next = removePhase(SAMPLE, 0)
    expect(parseAgentGraphPhases(next)).toEqual([
      expect.objectContaining({ key: 'investigate' }),
    ])
  })
})

describe('movePhase', () => {
  it('up で 1 つ前と入れ替える', () => {
    const next = movePhase(SAMPLE, 1, 'up')
    expect(parseAgentGraphPhases(next)?.map((p) => p.key)).toEqual([
      'investigate',
      'plan',
    ])
  })

  it('down で 1 つ後ろと入れ替える', () => {
    const next = movePhase(SAMPLE, 0, 'down')
    expect(parseAgentGraphPhases(next)?.map((p) => p.key)).toEqual([
      'investigate',
      'plan',
    ])
  })

  it('先頭を up しても何も起きない', () => {
    expect(parseAgentGraphPhases(movePhase(SAMPLE, 0, 'up'))).toEqual(
      parseAgentGraphPhases(SAMPLE),
    )
  })

  it('末尾を down しても何も起きない', () => {
    expect(parseAgentGraphPhases(movePhase(SAMPLE, 1, 'down'))).toEqual(
      parseAgentGraphPhases(SAMPLE),
    )
  })
})
