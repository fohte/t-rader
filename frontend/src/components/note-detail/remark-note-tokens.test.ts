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
    const tree = textTree('A [[stock:3436]] と [[indicator:USDJPY]] B')
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
              data: { hName: 'note-ref', hProperties: { token: 'stock:3436' } },
            },
            { type: 'text', value: ' と ' },
            {
              type: 'noteToken',
              data: {
                hName: 'note-ref',
                hProperties: { token: 'indicator:USDJPY' },
              },
            },
            { type: 'text', value: ' B' },
          ],
        },
      ],
    })
  })

  it('replaces [[anno:A2]] with a note-anno node', () => {
    const tree = textTree('シグナル [[anno:A2]] を見る')
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
              data: { hName: 'note-anno', hProperties: { annoId: 'A2' } },
            },
            { type: 'text', value: ' を見る' },
          ],
        },
      ],
    })
  })

  it('leaves unknown prefixes as literal text', () => {
    const tree = textTree('未知 [[foo:bar]] は素通り')
    remarkNoteTokens()(tree)
    expect(tree).toEqual(textTree('未知 [[foo:bar]] は素通り'))
  })

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
