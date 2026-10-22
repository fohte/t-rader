import type { Root } from 'mdast'
import { describe, expect, it } from 'vitest'

import { remarkNoteTokens } from '#components/note-detail/remark-note-tokens'

function textTree(value: string): Root {
  return {
    type: 'root',
    children: [{ type: 'paragraph', children: [{ type: 'text', value }] }],
  }
}

function paragraph(value: string) {
  return {
    type: 'paragraph' as const,
    children: [{ type: 'text' as const, value }],
  }
}

describe('remarkNoteTokens', () => {
  it('replaces prefixed refs with note-ref nodes', () => {
    const tree = textTree(
      'A [[stock:sample-code]] と [[indicator:demo-indicator]] と [[group:demo-axis/demo-group]] B',
    )
    remarkNoteTokens()(tree)
    expect(tree).toEqual({
      type: 'root',
      children: [
        {
          type: 'paragraph',
          children: [
            { type: 'text', value: 'A ' },
            {
              type: 'noteToken',
              data: {
                hName: 'note-ref',
                hProperties: { token: 'stock:sample-code' },
              },
            },
            { type: 'text', value: ' と ' },
            {
              type: 'noteToken',
              data: {
                hName: 'note-ref',
                hProperties: { token: 'indicator:demo-indicator' },
              },
            },
            { type: 'text', value: ' と ' },
            {
              type: 'noteToken',
              data: {
                hName: 'note-ref',
                hProperties: { token: 'group:demo-axis/demo-group' },
              },
            },
            { type: 'text', value: ' B' },
          ],
        },
      ],
    })
  })

  it('passes valuation and dated indicator details to evidence references', () => {
    const valuationToken = '[[valuation:US:FICTIONAL-A@2030-01-02:per]]'
    const observationToken = '[[indicator:demo-indicator@2030-01-02]]'
    const tree = textTree(
      `PER ${valuationToken}、demo-rate ${observationToken}、指標 [[indicator:demo-indicator]]`,
    )

    remarkNoteTokens({
      [valuationToken]: { value: 27.5 },
      [observationToken]: { value: 160.25 },
    })(tree)

    expect(tree).toEqual({
      type: 'root',
      children: [
        {
          type: 'paragraph',
          children: [
            { type: 'text', value: 'PER ' },
            {
              type: 'noteToken',
              data: {
                hName: 'note-valuation-reference',
                hProperties: {
                  instrumentId: 'US:FICTIONAL-A',
                  date: '2030-01-02',
                  field: 'per',
                  value: '27.5',
                },
              },
            },
            { type: 'text', value: '、demo-rate ' },
            {
              type: 'noteToken',
              data: {
                hName: 'note-indicator-reference',
                hProperties: {
                  indicatorId: 'demo-indicator',
                  date: '2030-01-02',
                  value: '160.25',
                },
              },
            },
            { type: 'text', value: '、指標 ' },
            {
              type: 'noteToken',
              data: {
                hName: 'note-ref',
                hProperties: { token: 'indicator:demo-indicator' },
              },
            },
          ],
        },
      ],
    })
  })

  it.each([
    {
      name: 'valuation',
      token: '[[valuation:demo-code@2030-01-02:per]]',
    },
    {
      name: 'dated indicator',
      token: '[[indicator:demo-indicator@2030-01-02]]',
    },
  ])('keeps an unresolved $name token visible', ({ token }) => {
    const tree = textTree(token)

    remarkNoteTokens()(tree)

    expect(tree).toEqual(textTree(token))
  })

  it('passes the evaluation ID and output name with a resolved value', () => {
    const token = '[[eval:00000000-0000-0000-0000-000000000101:result:rate]]'
    const tree = textTree(`結果 ${token}`)

    remarkNoteTokens({ [token]: { value: 256.75 } })(tree)

    expect(tree).toEqual({
      type: 'root',
      children: [
        {
          type: 'paragraph',
          children: [
            { type: 'text', value: '結果 ' },
            {
              type: 'noteToken',
              data: {
                hName: 'note-eval-reference',
                hProperties: {
                  evaluationId: '00000000-0000-0000-0000-000000000101',
                  outputName: 'result:rate',
                  value: '256.75',
                },
              },
            },
          ],
        },
      ],
    })
  })

  it('keeps an evaluation token visible when it has no resolved value', () => {
    const token = '[[eval:00000000-0000-0000-0000-000000000101:result]]'
    const tree = textTree(token)

    remarkNoteTokens()(tree)

    expect(tree).toEqual(textTree(token))
  })

  it.each([
    { name: 'invalid UUID', token: '[[eval:demo-id:result]]' },
    {
      name: 'empty output name',
      token: '[[eval:00000000-0000-0000-0000-000000000101:]]',
    },
  ])('keeps an invalid evaluation token visible ($name)', ({ token }) => {
    const tree = textTree(token)

    remarkNoteTokens({ [token]: { value: 256.75 } })(tree)

    expect(tree).toEqual(textTree(token))
  })

  it.each([
    { name: 'letter-leading', annoId: 'A2' },
    {
      name: 'digit-leading uuid',
      annoId: '0c2b6b3e-3f2a-4c9a-9e2a-3b7a2f6c9d1a',
    },
  ])(
    'replaces [[anno:$annoId]] with a note-anno node ($name)',
    ({ annoId }) => {
      const tree = textTree(`シグナル [[anno:${annoId}]] を見る`)
      remarkNoteTokens()(tree)
      expect(tree).toEqual({
        type: 'root',
        children: [
          {
            type: 'paragraph',
            children: [
              { type: 'text', value: 'シグナル ' },
              {
                type: 'noteToken',
                data: { hName: 'note-anno', hProperties: { annoId } },
              },
              { type: 'text', value: ' を見る' },
            ],
          },
        ],
      })
    },
  )

  it('leaves unknown prefixes as literal text', () => {
    const tree = textTree('未知 [[foo:bar]] は素通り')
    remarkNoteTokens()(tree)
    expect(tree).toEqual(textTree('未知 [[foo:bar]] は素通り'))
  })

  it('keeps a change value inline and appends its figure after the paragraph', () => {
    const token = '[[change:fictional-code@2030-01-02..2030-01-03:close]]'
    const position = {
      start: { line: 4, column: 1 },
      end: { line: 4, column: 28 },
    }
    const tree: Root = {
      type: 'root',
      children: [
        {
          ...paragraph(`期間変化 ${token}`),
          position,
        },
      ],
    }
    remarkNoteTokens({ [token]: { value: -12.1 } })(tree)
    expect(tree).toEqual({
      type: 'root',
      children: [
        {
          type: 'paragraph',
          children: [
            { type: 'text', value: '期間変化 ' },
            {
              type: 'noteToken',
              data: {
                hName: 'note-change-reference',
                hProperties: {
                  token,
                  kind: 'stock',
                  instrumentId: 'fictional-code',
                  start: '2030-01-02',
                  end: '2030-01-03',
                  field: 'close',
                  value: '-12.1',
                },
              },
            },
          ],
          position,
        },
        {
          type: 'noteChangeFigure',
          data: {
            hName: 'note-change-figure',
            hProperties: {
              kind: 'stock',
              instrumentId: 'fictional-code',
              start: '2030-01-02',
              end: '2030-01-03',
              field: 'close',
            },
          },
          position,
        },
      ],
    })
  })

  it('keeps an indicator change inline without appending a stock figure', () => {
    const token = '[[change:indicator:demo-indicator@2030-01-02..2030-01-03]]'
    const tree = textTree(`期間変化 ${token}`)
    remarkNoteTokens({ [token]: { value: 12.5 } })(tree)

    expect(tree).toEqual({
      type: 'root',
      children: [
        {
          type: 'paragraph',
          children: [
            { type: 'text', value: '期間変化 ' },
            {
              type: 'noteToken',
              data: {
                hName: 'note-change-reference',
                hProperties: {
                  token,
                  kind: 'indicator',
                  indicatorId: 'demo-indicator',
                  start: '2030-01-02',
                  end: '2030-01-03',
                  value: '12.5',
                },
              },
            },
          ],
        },
      ],
    })
  })

  it.each([
    {
      name: 'an impossible date',
      token: '[[change:fictional-code@2030-02-30..2030-03-01:close]]',
    },
    {
      name: 'a reversed date range',
      token: '[[change:fictional-code@2030-01-03..2030-01-02:close]]',
    },
  ])('leaves $name as literal text without a figure', ({ token }) => {
    const tree = textTree(`期間変化 ${token}`)
    remarkNoteTokens()(tree)
    expect(tree).toEqual(textTree(`期間変化 ${token}`))
  })

  it('appends figures inside blockquotes and list items', () => {
    const token = '[[change:fictional-code@2030-01-02..2030-01-03:close]]'
    const tree: Root = {
      type: 'root',
      children: [
        { type: 'blockquote', children: [paragraph(token)] },
        {
          type: 'list',
          ordered: false,
          children: [
            {
              type: 'listItem',
              children: [paragraph(token)],
            },
          ],
        },
      ],
    }
    remarkNoteTokens()(tree)
    expect(tree).toEqual({
      type: 'root',
      children: [
        {
          type: 'blockquote',
          children: [
            {
              type: 'paragraph',
              children: [
                {
                  type: 'noteToken',
                  data: {
                    hName: 'note-change-reference',
                    hProperties: {
                      token,
                      kind: 'stock',
                      instrumentId: 'fictional-code',
                      start: '2030-01-02',
                      end: '2030-01-03',
                      field: 'close',
                    },
                  },
                },
              ],
            },
            {
              type: 'noteChangeFigure',
              data: {
                hName: 'note-change-figure',
                hProperties: {
                  kind: 'stock',
                  instrumentId: 'fictional-code',
                  start: '2030-01-02',
                  end: '2030-01-03',
                  field: 'close',
                },
              },
            },
          ],
        },
        {
          type: 'list',
          ordered: false,
          children: [
            {
              type: 'listItem',
              children: [
                {
                  type: 'paragraph',
                  children: [
                    {
                      type: 'noteToken',
                      data: {
                        hName: 'note-change-reference',
                        hProperties: {
                          token,
                          kind: 'stock',
                          instrumentId: 'fictional-code',
                          start: '2030-01-02',
                          end: '2030-01-03',
                          field: 'close',
                        },
                      },
                    },
                  ],
                },
                {
                  type: 'noteChangeFigure',
                  data: {
                    hName: 'note-change-figure',
                    hProperties: {
                      kind: 'stock',
                      instrumentId: 'fictional-code',
                      start: '2030-01-02',
                      end: '2030-01-03',
                      field: 'close',
                    },
                  },
                },
              ],
            },
          ],
        },
      ],
    })
  })

  it('passes the instrument, date, and field from a resolved price token', () => {
    const token = '[[price:US:FICTIONAL-A@2030-01-02:high]]'
    const tree = textTree(`高値 ${token}`)
    remarkNoteTokens({ [token]: { value: 1234.5 } })(tree)

    expect(tree).toEqual({
      type: 'root',
      children: [
        {
          type: 'paragraph',
          children: [
            { type: 'text', value: '高値 ' },
            {
              type: 'noteToken',
              data: {
                hName: 'note-price-reference',
                hProperties: {
                  token,
                  instrumentId: 'US:FICTIONAL-A',
                  date: '2030-01-02',
                  field: 'high',
                  value: '1234.5',
                },
              },
            },
          ],
        },
      ],
    })
  })

  it.each(['sector', 'theme'] as const)(
    'leaves the removed %s reference kind as literal text',
    (kind) => {
      const token = `[[${kind}:demo-value]]`
      const tree = textTree(`旧参照 ${token} は素通り`)
      remarkNoteTokens()(tree)
      expect(tree).toEqual(textTree(`旧参照 ${token} は素通り`))
    },
  )

  it('replaces a paragraph consisting solely of [[graph:g1]] with a note-graph block', () => {
    const tree = textTree('[[graph:g1]]')
    remarkNoteTokens()(tree)
    expect(tree).toEqual({
      type: 'root',
      children: [
        {
          type: 'noteGraphBlock',
          data: { hName: 'note-graph', hProperties: { graphId: 'g1' } },
        },
      ],
    })
  })

  it('keeps a graph block source position for line comments', () => {
    const position = {
      start: { line: 7, column: 1 },
      end: { line: 7, column: 13 },
    }
    const tree: Root = {
      type: 'root',
      children: [
        {
          type: 'paragraph',
          position,
          children: [{ type: 'text', value: '[[graph:g1]]' }],
        },
      ],
    }

    remarkNoteTokens()(tree)

    expect(tree).toEqual({
      type: 'root',
      children: [
        {
          type: 'noteGraphBlock',
          data: { hName: 'note-graph', hProperties: { graphId: 'g1' } },
          position,
        },
      ],
    })
  })

  it('leaves [[graph:g1]] mixed with other text in the same paragraph untouched', () => {
    const tree = textTree('見る [[graph:g1]] こと')
    remarkNoteTokens()(tree)
    expect(tree).toEqual(textTree('見る [[graph:g1]] こと'))
  })
})
