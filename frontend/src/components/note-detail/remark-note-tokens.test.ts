import type { Root } from 'mdast'
import { describe, expect, it } from 'vitest'

import { remarkNoteTokens } from '#components/note-detail/remark-note-tokens'

function textTree(value: string): Root {
  return {
    type: 'root',
    children: [{ type: 'paragraph', children: [{ type: 'text', value }] }],
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
    const tree = textTree(`期間変化 ${token}`)
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
                hProperties: { token, value: '-12.1' },
              },
            },
          ],
        },
        {
          type: 'noteChangeFigure',
          data: { hName: 'note-change-figure', hProperties: { token } },
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

  it('leaves [[graph:g1]] mixed with other text in the same paragraph untouched', () => {
    const tree = textTree('見る [[graph:g1]] こと')
    remarkNoteTokens()(tree)
    expect(tree).toEqual(textTree('見る [[graph:g1]] こと'))
  })
})
