import { describe, expect, it } from 'vitest'

import { checkOutputSchemaText } from '#components/strategy-settings/agent-graph/output-schema-check'

describe('checkOutputSchemaText', () => {
  it('空文字列は空の output として扱う', () => {
    expect(checkOutputSchemaText('')).toEqual({ output: {}, issues: [] })
  })

  it('空白のみの文字列は空の output として扱う', () => {
    expect(checkOutputSchemaText('   \n')).toEqual({ output: {}, issues: [] })
  })

  it('YAML 構文エラーは output: null で 1 件の issue を返す', () => {
    expect(checkOutputSchemaText('foo: [')).toEqual({
      output: null,
      issues: [
        {
          message: expect.any(String),
          line: expect.any(Number),
          column: expect.any(Number),
        },
      ],
    })
  })

  it('object 配列 + プリミティブ配列 + required を持つ有効な output は issue なし', () => {
    const text = `hypotheses:
  type: array
  description: 検証すべき仮説
  items:
    title:
      type: string
    rationale:
      type: string
    checks:
      type: array
      items:
        type: string
  required: [title, rationale]
`
    expect(checkOutputSchemaText(text)).toEqual({
      issues: [],
      output: {
        hypotheses: {
          type: 'array',
          description: '検証すべき仮説',
          items: {
            title: { type: 'string' },
            rationale: { type: 'string' },
            checks: { type: 'array', items: { type: 'string' } },
          },
          required: ['title', 'rationale'],
        },
      },
    })
  })

  it('enum フィールドが配列でなければ issue を出すが output は commit される', () => {
    const text = `verdict:
  enum: supported
`
    expect(checkOutputSchemaText(text)).toEqual({
      issues: [
        {
          message: 'verdict.enum は配列である必要があります',
          line: 2,
          column: 9,
        },
      ],
      output: { verdict: { enum: 'supported' } },
    })
  })

  it('items 内の `required` キーは予約されず、ただのフィールドとして扱われる', () => {
    const text = `hypotheses:
  type: array
  items:
    title:
      type: string
    required:
      type: string
`
    expect(checkOutputSchemaText(text)).toEqual({
      issues: [],
      output: {
        hypotheses: {
          type: 'array',
          items: {
            title: { type: 'string' },
            required: { type: 'string' },
          },
        },
      },
    })
  })

  it('items を持たないフィールドの required は実行時に無視されるため issue を出す', () => {
    const text = `verdict:
  type: string
  required: [verdict]
`
    expect(checkOutputSchemaText(text)).toEqual({
      issues: [
        {
          message:
            'verdict.required は items がオブジェクト配列の場合のみ有効です (ここでは無視されます)',
          line: 3,
          column: 13,
        },
      ],
      output: { verdict: { type: 'string', required: ['verdict'] } },
    })
  })

  it('items がプリミティブ配列のフィールドの required は実行時に無視されるため issue を出す', () => {
    const text = `checks:
  type: array
  items:
    type: string
  required: [checks]
`
    expect(checkOutputSchemaText(text)).toEqual({
      issues: [
        {
          message:
            'checks.required は items がオブジェクト配列の場合のみ有効です (ここでは無視されます)',
          line: 5,
          column: 13,
        },
      ],
      output: {
        checks: {
          type: 'array',
          items: { type: 'string' },
          required: ['checks'],
        },
      },
    })
  })

  it('トップレベルの required が文字列配列でなければ issue を出す', () => {
    const text = `verdict:
  type: string
required: verdict
`
    expect(checkOutputSchemaText(text)).toEqual({
      issues: [
        {
          message: 'required は文字列の配列である必要があります',
          line: 3,
          column: 11,
        },
      ],
      output: { verdict: { type: 'string' }, required: 'verdict' },
    })
  })

  it('トップレベルがマップでなければ issue を出す', () => {
    expect(checkOutputSchemaText('- foo\n- bar\n')).toEqual({
      issues: [
        {
          message:
            'output はフィールド名 → スキーマのマップである必要があります',
          line: 1,
          column: 1,
        },
      ],
      output: {},
    })
  })

  it('フィールドスキーマがオブジェクトでなければ issue を出す', () => {
    expect(checkOutputSchemaText('verdict: supported\n')).toEqual({
      issues: [
        {
          message: 'verdict はオブジェクトである必要があります',
          line: 1,
          column: 10,
        },
      ],
      output: { verdict: 'supported' },
    })
  })

  it('items がオブジェクトでなければ issue を出す', () => {
    const text = `hypotheses:
  type: array
  items: title
`
    expect(checkOutputSchemaText(text)).toEqual({
      issues: [
        {
          message: 'hypotheses.items はオブジェクトである必要があります',
          line: 3,
          column: 10,
        },
      ],
      output: { hypotheses: { type: 'array', items: 'title' } },
    })
  })
})
