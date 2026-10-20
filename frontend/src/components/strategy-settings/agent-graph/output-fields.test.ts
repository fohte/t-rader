import { describe, expect, it } from 'vitest'

import {
  getForEachOptions,
  getLabelFieldOptions,
} from '#components/strategy-settings/agent-graph/output-fields'
import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'

const PLAN: AgentGraphPhaseForm = {
  key: 'plan',
  label: '見出し案',
  model: 'fictional-model-orbit',
  prompt: 'テーマを読みやすい節に分ける',
  skills: [],
  tools: [],
  output: {
    sections: {
      type: 'array',
      items: {
        title: { type: 'string' },
        description: { type: 'string' },
        key_points: { type: 'array', items: { type: 'string' } },
      },
      required: ['title', 'description'],
    },
    overview: { type: 'string' },
  },
}

const COMPOSE: AgentGraphPhaseForm = {
  key: 'compose',
  label: '節の作成',
  model: 'fictional-model-lantern',
  prompt: '割り当てられた見出しに沿って、説明文を作成する。',
  forEach: 'plan.sections',
  skills: [],
  tools: [],
  output: {
    abstract: { enum: ['short', 'long'] },
  },
}

describe('getForEachOptions', () => {
  it('前段フェーズの output の配列フィールドだけを列挙する', () => {
    expect(getForEachOptions([PLAN, COMPOSE], 1)).toEqual([
      { value: 'plan.sections', label: '見出し案 → sections[] の要素ごと' },
    ])
  })

  it('自身より後ろのフェーズは対象にしない', () => {
    expect(getForEachOptions([PLAN, COMPOSE], 0)).toEqual([])
  })

  it('output が空のフェーズしかなければ空配列を返す', () => {
    const noOutput: AgentGraphPhaseForm = { ...PLAN, output: {} }
    expect(getForEachOptions([noOutput], 1)).toEqual([])
  })
})

describe('getLabelFieldOptions', () => {
  it('for_each が指す配列フィールドの items から string 型の property だけを列挙する', () => {
    expect(getLabelFieldOptions([PLAN, COMPOSE], 'plan.sections')).toEqual([
      'title',
      'description',
    ])
  })

  it('for_each が未設定なら空配列を返す', () => {
    expect(getLabelFieldOptions([PLAN, COMPOSE], undefined)).toEqual([])
  })

  it('参照先フェーズが存在しなければ空配列を返す', () => {
    expect(getLabelFieldOptions([PLAN], 'missing.sections')).toEqual([])
  })

  it('参照先フィールドが存在しなければ空配列を返す', () => {
    expect(getLabelFieldOptions([PLAN], 'plan.missing_section')).toEqual([])
  })

  it('items がプリミティブ配列 (string 型の property を持たない) なら空配列を返す', () => {
    const primitiveArrayOnly: AgentGraphPhaseForm = {
      ...PLAN,
      output: {
        key_points: { type: 'array', items: { type: 'string' } },
      },
    }
    expect(
      getLabelFieldOptions([primitiveArrayOnly], 'plan.key_points'),
    ).toEqual([])
  })
})
