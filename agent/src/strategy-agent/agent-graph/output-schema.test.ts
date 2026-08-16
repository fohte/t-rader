import { describe, expect, it } from 'vitest'

import { buildOutputJsonSchema } from '#strategy-agent/agent-graph/output-schema'

describe('buildOutputJsonSchema', () => {
  it('returns an empty object schema for an empty output config', () => {
    expect(buildOutputJsonSchema({})).toEqual({
      type: 'object',
      properties: {},
    })
  })

  it('builds an object array field with required as a sibling of items, and a primitive array sub-field', () => {
    const output = {
      hypotheses: {
        type: 'array',
        description: '検証すべき仮説。2-4件',
        items: {
          title: { type: 'string', description: '仮説を1文で言い切ったもの' },
          rationale: { type: 'string', description: 'なぜその仮説が立つか' },
          checks: {
            type: 'array',
            description: '棄却できる観測',
            items: { type: 'string' },
          },
        },
        required: ['title', 'rationale'],
      },
    }

    expect(buildOutputJsonSchema(output)).toEqual({
      type: 'object',
      properties: {
        hypotheses: {
          type: 'array',
          description: '検証すべき仮説。2-4件',
          items: {
            type: 'object',
            properties: {
              title: {
                type: 'string',
                description: '仮説を1文で言い切ったもの',
              },
              rationale: {
                type: 'string',
                description: 'なぜその仮説が立つか',
              },
              checks: {
                type: 'array',
                description: '棄却できる観測',
                items: { type: 'string' },
              },
            },
            required: ['title', 'rationale'],
          },
        },
      },
    })
  })

  it('applies a top-level required list sibling to the output fields themselves', () => {
    const output = {
      verdict: { type: 'string' },
      summary: { type: 'string' },
      required: ['verdict'],
    }

    expect(buildOutputJsonSchema(output)).toEqual({
      type: 'object',
      properties: {
        verdict: { type: 'string' },
        summary: { type: 'string' },
      },
      required: ['verdict'],
    })
  })

  it('passes through an enum field with no type key', () => {
    const output = {
      verdict: {
        enum: ['supported', 'rejected', 'inconclusive'],
        description: 'checks を当てた結果',
      },
    }

    expect(buildOutputJsonSchema(output)).toEqual({
      type: 'object',
      properties: {
        verdict: {
          enum: ['supported', 'rejected', 'inconclusive'],
          description: 'checks を当てた結果',
        },
      },
    })
  })
})
