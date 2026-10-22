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
    const text = `sample_field:
  type: array
  description: sample array
  items:
    name:
      type: string
    value:
      type: string
    values:
      type: array
      items:
        type: string
  required: [name, value]
`
    expect(checkOutputSchemaText(text)).toEqual({
      issues: [],
      output: {
        sample_field: {
          type: 'array',
          description: 'sample array',
          items: {
            name: { type: 'string' },
            value: { type: 'string' },
            values: { type: 'array', items: { type: 'string' } },
          },
          required: ['name', 'value'],
        },
      },
    })
  })

  it('type-less なネストした object とその required を有効として扱う', () => {
    const text = `sample_object:
  required: [field_alpha]
  field_alpha:
    required: [items]
    items:
      type: array
      items:
        type: string
`

    expect(checkOutputSchemaText(text)).toEqual({
      issues: [],
      output: {
        sample_object: {
          required: ['field_alpha'],
          field_alpha: {
            required: ['items'],
            items: {
              type: 'array',
              items: { type: 'string' },
            },
          },
        },
      },
    })
  })

  it('ネストした object 内で items に書かれた required を有効として扱う', () => {
    const text = `sample_object:
  records:
    type: array
    items:
      required: [field_alpha, field_beta]
      field_alpha:
        type: string
      field_beta:
        type: number
`

    expect(checkOutputSchemaText(text)).toEqual({
      issues: [],
      output: {
        sample_object: {
          records: {
            type: 'array',
            items: {
              required: ['field_alpha', 'field_beta'],
              field_alpha: { type: 'string' },
              field_beta: { type: 'number' },
            },
          },
        },
      },
    })
  })

  it('items 内の不正な required は issue を 1 件だけ返す', () => {
    const text = `sample_object:
  records:
    type: array
    items:
      required: field_alpha
      field_alpha:
        type: string
`

    expect(checkOutputSchemaText(text)).toEqual({
      issues: [
        {
          message:
            'sample_object.records.items.required は文字列の配列である必要があります',
          line: 5,
          column: 17,
        },
      ],
      output: {
        sample_object: {
          records: {
            type: 'array',
            items: {
              required: 'field_alpha',
              field_alpha: { type: 'string' },
            },
          },
        },
      },
    })
  })

  it('enum フィールドが配列でなければ issue を出すが output は commit される', () => {
    const text = `sample_field:
  enum: sample-value
`
    expect(checkOutputSchemaText(text)).toEqual({
      issues: [
        {
          message: 'sample_field.enum は配列である必要があります',
          line: 2,
          column: 9,
        },
      ],
      output: { sample_field: { enum: 'sample-value' } },
    })
  })

  it('items 内の `required` キーは予約されず、ただのフィールドとして扱われる', () => {
    const text = `sample_field:
  type: array
  items:
    name:
      type: string
    required:
      type: string
`
    expect(checkOutputSchemaText(text)).toEqual({
      issues: [],
      output: {
        sample_field: {
          type: 'array',
          items: {
            name: { type: 'string' },
            required: { type: 'string' },
          },
        },
      },
    })
  })

  it('items を持たないフィールドの required は実行時に無視されるため issue を出す', () => {
    const text = `sample_field:
  type: string
  required: [sample_field]
`
    expect(checkOutputSchemaText(text)).toEqual({
      issues: [
        {
          message:
            'sample_field.required は items がオブジェクト配列の場合のみ有効です (ここでは無視されます)',
          line: 3,
          column: 13,
        },
      ],
      output: { sample_field: { type: 'string', required: ['sample_field'] } },
    })
  })

  it('items がプリミティブ配列のフィールドの required は実行時に無視されるため issue を出す', () => {
    const text = `sample_values:
  type: array
  items:
    type: string
  required: [sample_values]
`
    expect(checkOutputSchemaText(text)).toEqual({
      issues: [
        {
          message:
            'sample_values.required は items がオブジェクト配列の場合のみ有効です (ここでは無視されます)',
          line: 5,
          column: 13,
        },
      ],
      output: {
        sample_values: {
          type: 'array',
          items: { type: 'string' },
          required: ['sample_values'],
        },
      },
    })
  })

  it('トップレベルの required が文字列配列でなければ issue を出す', () => {
    const text = `sample_field:
  type: string
required: sample_field
`
    expect(checkOutputSchemaText(text)).toEqual({
      issues: [
        {
          message: 'required は文字列の配列である必要があります',
          line: 3,
          column: 11,
        },
      ],
      output: { sample_field: { type: 'string' }, required: 'sample_field' },
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
    expect(checkOutputSchemaText('sample_field: sample-value\n')).toEqual({
      issues: [
        {
          message: 'sample_field はオブジェクトである必要があります',
          line: 1,
          column: 15,
        },
      ],
      output: { sample_field: 'sample-value' },
    })
  })

  it('items がオブジェクトでなければ issue を出す', () => {
    const text = `sample_field:
  type: array
  items: name
`
    expect(checkOutputSchemaText(text)).toEqual({
      issues: [
        {
          message: 'sample_field.items はオブジェクトである必要があります',
          line: 3,
          column: 10,
        },
      ],
      output: { sample_field: { type: 'array', items: 'name' } },
    })
  })
})
