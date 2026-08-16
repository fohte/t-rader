import { describe, expect, it } from 'vitest'

import {
  getForEachOptions,
  getLabelFieldOptions,
} from '#components/strategy-settings/agent-graph/output-fields'
import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'

const PLAN: AgentGraphPhaseForm = {
  key: 'plan',
  label: '調査計画',
  model: 'claude-opus-4',
  prompt: '仮説を立てよ',
  skills: [],
  tools: [],
  output: {
    hypotheses: {
      type: 'array',
      items: {
        title: { type: 'string' },
        rationale: { type: 'string' },
        checks: { type: 'array', items: { type: 'string' } },
      },
      required: ['title', 'rationale'],
    },
    summary: { type: 'string' },
  },
}

const INVESTIGATE: AgentGraphPhaseForm = {
  key: 'investigate',
  label: '仮説の調査',
  model: 'deepseek-v4-flash',
  prompt: '割り当てられた仮説を検証せよ',
  forEach: 'plan.hypotheses',
  skills: [],
  tools: [],
  output: {
    verdict: { enum: ['supported', 'rejected'] },
  },
}

describe('getForEachOptions', () => {
  it('前段フェーズの output の配列フィールドだけを列挙する', () => {
    expect(getForEachOptions([PLAN, INVESTIGATE], 1)).toEqual([
      { value: 'plan.hypotheses', label: '調査計画 → hypotheses[] の要素ごと' },
    ])
  })

  it('自身より後ろのフェーズは対象にしない', () => {
    expect(getForEachOptions([PLAN, INVESTIGATE], 0)).toEqual([])
  })

  it('output が空のフェーズしかなければ空配列を返す', () => {
    const noOutput: AgentGraphPhaseForm = { ...PLAN, output: {} }
    expect(getForEachOptions([noOutput], 1)).toEqual([])
  })
})

describe('getLabelFieldOptions', () => {
  it('for_each が指す配列フィールドの items から string 型の property だけを列挙する', () => {
    expect(
      getLabelFieldOptions([PLAN, INVESTIGATE], 'plan.hypotheses'),
    ).toEqual(['title', 'rationale'])
  })

  it('for_each が未設定なら空配列を返す', () => {
    expect(getLabelFieldOptions([PLAN, INVESTIGATE], undefined)).toEqual([])
  })

  it('参照先フェーズが存在しなければ空配列を返す', () => {
    expect(getLabelFieldOptions([PLAN], 'missing.hypotheses')).toEqual([])
  })

  it('参照先フィールドが存在しなければ空配列を返す', () => {
    expect(getLabelFieldOptions([PLAN], 'plan.missing_field')).toEqual([])
  })

  it('items がプリミティブ配列 (string 型の property を持たない) なら空配列を返す', () => {
    const primitiveArrayOnly: AgentGraphPhaseForm = {
      ...PLAN,
      output: {
        checks: { type: 'array', items: { type: 'string' } },
      },
    }
    expect(getLabelFieldOptions([primitiveArrayOnly], 'plan.checks')).toEqual(
      [],
    )
  })
})
