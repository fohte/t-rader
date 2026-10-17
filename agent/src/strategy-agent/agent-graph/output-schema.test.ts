import { describe, expect, it } from 'vitest'

import { buildOutputJsonSchema } from '#strategy-agent/agent-graph/output-schema'

const STRUCTURED_OUTPUT_TOOL_DESCRIPTION =
  "Tool for extracting structured output from the model's response."

describe('buildOutputJsonSchema', () => {
  it('returns an empty object schema for an empty output config', () => {
    expect(buildOutputJsonSchema({})).toEqual({
      type: 'object',
      properties: {},
      description: STRUCTURED_OUTPUT_TOOL_DESCRIPTION,
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
      description: STRUCTURED_OUTPUT_TOOL_DESCRIPTION,
    })
  })

  it('recursively converts type-less object fields and object array items', () => {
    const output = {
      record: {
        required: ['label', 'details'],
        label: { type: 'string' },
        details: {
          required: ['entries', 'items'],
          items: { type: 'array', items: { type: 'string' } },
          entries: {
            type: 'array',
            items: {
              code: { type: 'string' },
              attributes: {
                enabled: { type: 'boolean' },
                scores: { type: 'array', items: { type: 'number' } },
              },
            },
            required: ['code', 'attributes'],
          },
        },
      },
    }

    expect(buildOutputJsonSchema(output)).toEqual({
      type: 'object',
      properties: {
        record: {
          type: 'object',
          properties: {
            label: { type: 'string' },
            details: {
              type: 'object',
              properties: {
                items: {
                  type: 'array',
                  items: { type: 'string' },
                },
                entries: {
                  type: 'array',
                  items: {
                    type: 'object',
                    properties: {
                      code: { type: 'string' },
                      attributes: {
                        type: 'object',
                        properties: {
                          enabled: { type: 'boolean' },
                          scores: {
                            type: 'array',
                            items: { type: 'number' },
                          },
                        },
                      },
                    },
                    required: ['code', 'attributes'],
                  },
                },
              },
              required: ['entries', 'items'],
            },
          },
          required: ['label', 'details'],
        },
      },
      description: STRUCTURED_OUTPUT_TOOL_DESCRIPTION,
    })
  })

  it('preserves a type-less enum schema in array items', () => {
    const output = {
      labels: {
        type: 'array',
        items: { enum: ['north', 'south'] },
      },
    }

    expect(buildOutputJsonSchema(output)).toEqual({
      type: 'object',
      properties: {
        labels: {
          type: 'array',
          items: { enum: ['north', 'south'] },
        },
      },
      description: STRUCTURED_OUTPUT_TOOL_DESCRIPTION,
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
      description: STRUCTURED_OUTPUT_TOOL_DESCRIPTION,
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
      description: STRUCTURED_OUTPUT_TOOL_DESCRIPTION,
    })
  })
})
