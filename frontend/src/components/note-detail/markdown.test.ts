import { describe, expect, it } from 'vitest'

import { parseInline, parseMarkdown } from '#components/note-detail/markdown'

describe('parseInline', () => {
  it('extracts prefix refs as ref tokens', () => {
    expect(parseInline('A [[stock:3436]] と [[indicator:USDJPY]] B')).toEqual([
      { kind: 'text', value: 'A ' },
      { kind: 'ref', token: 'stock:3436' },
      { kind: 'text', value: ' と ' },
      { kind: 'ref', token: 'indicator:USDJPY' },
      { kind: 'text', value: ' B' },
    ])
  })

  it('treats [[anno:A2]] as anno chip', () => {
    expect(parseInline('シグナル [[anno:A2]] を見る')).toEqual([
      { kind: 'text', value: 'シグナル ' },
      { kind: 'anno', id: 'A2' },
      { kind: 'text', value: ' を見る' },
    ])
  })

  it('keeps unknown prefix as text', () => {
    expect(parseInline('未知 [[foo:bar]] は素通り')).toEqual([
      { kind: 'text', value: '未知 ' },
      { kind: 'text', value: '[[foo:bar]]' },
      { kind: 'text', value: ' は素通り' },
    ])
  })

  it('handles bold/italic/code inline', () => {
    expect(parseInline('a **b** c *d* e `f`')).toEqual([
      { kind: 'text', value: 'a ' },
      { kind: 'bold', value: 'b' },
      { kind: 'text', value: ' c ' },
      { kind: 'italic', value: 'd' },
      { kind: 'text', value: ' e ' },
      { kind: 'code', value: 'f' },
    ])
  })
})

describe('parseMarkdown', () => {
  it('splits headings, lists, paragraphs', () => {
    const src = '# Title\n\n本文 1 行目\n\n## H2\n\n- item A\n- item B\n'
    const blocks = parseMarkdown(src)
    expect(blocks.map((b) => b.kind)).toEqual(['h1', 'p', 'h2', 'ul'])
  })

  it('captures code fences with lang', () => {
    const src = '```python\nprint(1)\n```\n'
    const blocks = parseMarkdown(src)
    expect(blocks).toEqual([
      { kind: 'code', value: 'print(1)', lang: 'python' },
    ])
  })

  it('parses blockquote', () => {
    const blocks = parseMarkdown('> 引用文 [[stock:7203]] 込み\n')
    expect(blocks).toHaveLength(1)
    const [b] = blocks
    if (b?.kind !== 'quote') throw new Error('expected quote block')
    expect(b.inline.some((t) => t.kind === 'ref')).toBe(true)
  })
})
