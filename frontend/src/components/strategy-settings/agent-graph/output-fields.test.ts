import { describe, expect, it } from 'vitest'

import {
  getForEachOptions,
  getLabelFieldOptions,
} from '#components/strategy-settings/agent-graph/output-fields'
import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'

const ALPHA: AgentGraphPhaseForm = {
  key: 'alpha',
  label: 'サンプル',
  model: 'fictional-model-orbit',
  prompt: 'sample prompt',
  skills: [],
  tools: [],
  output: {
    records: {
      type: 'array',
      items: {
        name: { type: 'string' },
        value: { type: 'string' },
        entries: { type: 'array', items: { type: 'string' } },
      },
      required: ['name', 'value'],
    },
    sample_field: { type: 'string' },
  },
}

const BETA: AgentGraphPhaseForm = {
  key: 'beta',
  label: '次の段階',
  model: 'fictional-model-lantern',
  prompt: 'sample prompt',
  forEach: 'alpha.records',
  skills: [],
  tools: [],
  output: {
    sample_field: { enum: ['short', 'long'] },
  },
}

describe('getForEachOptions', () => {
  it('前段フェーズの output の配列フィールドだけを列挙する', () => {
    expect(getForEachOptions([ALPHA, BETA], 1)).toEqual([
      { value: 'alpha.records', label: 'サンプル → records[] の要素ごと' },
    ])
  })

  it('自身より後ろのフェーズは対象にしない', () => {
    expect(getForEachOptions([ALPHA, BETA], 0)).toEqual([])
  })

  it('output が空のフェーズしかなければ空配列を返す', () => {
    const noOutput: AgentGraphPhaseForm = { ...ALPHA, output: {} }
    expect(getForEachOptions([noOutput], 1)).toEqual([])
  })
})

describe('getLabelFieldOptions', () => {
  it('for_each が指す配列フィールドの items から string 型の property だけを列挙する', () => {
    expect(getLabelFieldOptions([ALPHA, BETA], 'alpha.records')).toEqual([
      'name',
      'value',
    ])
  })

  it('for_each が未設定なら空配列を返す', () => {
    expect(getLabelFieldOptions([ALPHA, BETA], undefined)).toEqual([])
  })

  it('参照先フェーズが存在しなければ空配列を返す', () => {
    expect(getLabelFieldOptions([ALPHA], 'missing.records')).toEqual([])
  })

  it('参照先フィールドが存在しなければ空配列を返す', () => {
    expect(getLabelFieldOptions([ALPHA], 'alpha.missing_field')).toEqual([])
  })

  it('items がプリミティブ配列 (string 型の property を持たない) なら空配列を返す', () => {
    const primitiveArrayOnly: AgentGraphPhaseForm = {
      ...ALPHA,
      output: {
        entries: { type: 'array', items: { type: 'string' } },
      },
    }
    expect(getLabelFieldOptions([primitiveArrayOnly], 'alpha.entries')).toEqual(
      [],
    )
  })
})
