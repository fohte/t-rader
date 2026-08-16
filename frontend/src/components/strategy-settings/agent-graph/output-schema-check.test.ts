import { describe, expect, it } from 'vitest'

import { checkOutputSchemaText } from '#components/strategy-settings/agent-graph/output-schema-check'

describe('checkOutputSchemaText', () => {
  it('空文字列は空の output として扱う', () => {
    expect(checkOutputSchemaText('')).toEqual({ output: {}, issues: [] })
    expect(checkOutputSchemaText('   \n')).toEqual({ output: {}, issues: [] })
  })

  it('YAML 構文エラーは output: null で 1 件の issue を返す', () => {
    const result = checkOutputSchemaText('foo: [')
    expect(result.output).toBeNull()
    expect(result.issues).toHaveLength(1)
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
    const result = checkOutputSchemaText(text)
    expect(result.issues).toEqual([])
    expect(result.output).toEqual({
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
    })
  })

  it('enum フィールドが配列でなければ issue を出すが output は commit される', () => {
    const text = `verdict:
  enum: supported
`
    const result = checkOutputSchemaText(text)
    expect(result.issues).toEqual([
      {
        message: 'verdict.enum は配列である必要があります',
        line: 2,
        column: 9,
      },
    ])
    expect(result.output).toEqual({ verdict: { enum: 'supported' } })
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
    const result = checkOutputSchemaText(text)
    expect(result.issues).toEqual([])
    expect(result.output).toEqual({
      hypotheses: {
        type: 'array',
        items: {
          title: { type: 'string' },
          required: { type: 'string' },
        },
      },
    })
  })

  it('トップレベルの required が文字列配列でなければ issue を出す', () => {
    const text = `verdict:
  type: string
required: verdict
`
    const result = checkOutputSchemaText(text)
    expect(result.issues).toEqual([
      {
        message: 'required は文字列の配列である必要があります',
        line: 3,
        column: 11,
      },
    ])
  })

  it('トップレベルがマップでなければ issue を出す', () => {
    const result = checkOutputSchemaText('- foo\n- bar\n')
    expect(result.issues).toEqual([
      {
        message: 'output はフィールド名 → スキーマのマップである必要があります',
        line: 1,
        column: 1,
      },
    ])
    expect(result.output).toEqual({})
  })

  it('フィールドスキーマがオブジェクトでなければ issue を出す', () => {
    const result = checkOutputSchemaText('verdict: supported\n')
    expect(result.issues).toEqual([
      {
        message: 'verdict はオブジェクトである必要があります',
        line: 1,
        column: 10,
      },
    ])
  })

  it('items がオブジェクトでなければ issue を出す', () => {
    const text = `hypotheses:
  type: array
  items: title
`
    const result = checkOutputSchemaText(text)
    expect(result.issues).toEqual([
      {
        message: 'hypotheses.items はオブジェクトである必要があります',
        line: 3,
        column: 10,
      },
    ])
  })
})
