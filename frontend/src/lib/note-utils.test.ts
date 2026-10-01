import { describe, expect, it } from 'vitest'

import { buildSnippet, extractRefs, formatRelative } from '#lib/note-utils'

describe('extractRefs', () => {
  it('returns frontmatter refs when present', () => {
    expect(
      extractRefs({
        frontmatter_json: {
          refs: ['stock:demo-code', 'indicator:demo-indicator'],
        },
        body_md: '本文 [[stock:sample-code]]',
      }),
    ).toEqual(['stock:demo-code', 'indicator:demo-indicator'])
  })

  it('falls back to body scan when frontmatter has no refs', () => {
    expect(
      extractRefs({
        frontmatter_json: {},
        body_md:
          '[[stock:sample-code]] と [[indicator:demo-indicator]] と [[group:demo-axis/demo-group]] を見る。[[theme:demo-topic]] は旧形式。',
      }),
    ).toEqual([
      'stock:sample-code',
      'indicator:demo-indicator',
      'group:demo-axis/demo-group',
    ])
  })

  it('deduplicates body refs', () => {
    expect(
      extractRefs({
        frontmatter_json: {},
        body_md: '[[stock:sample-code]] と再掲 [[stock:sample-code]]',
      }),
    ).toEqual(['stock:sample-code'])
  })

  it('filters non-string values from frontmatter refs', () => {
    expect(
      extractRefs({
        frontmatter_json: { refs: ['stock:demo-code', 42, null] },
        body_md: '',
      }),
    ).toEqual(['stock:demo-code'])
  })
})

describe('buildSnippet', () => {
  it('strips markdown and ref syntax', () => {
    expect(buildSnippet('## 見出し\n*強調* と [[stock:sample-code]]')).toBe(
      '見出し 強調 と sample-code',
    )
  })

  it('truncates with ellipsis when over max', () => {
    const long = 'あ'.repeat(200)
    const snippet = buildSnippet(long, 50)
    expect(snippet.endsWith('…')).toBe(true)
    expect(snippet.length).toBe(51)
  })
})

describe('formatRelative', () => {
  const now = new Date('2026-06-07T12:00:00Z').getTime()

  it('returns "たった今" for sub-second diffs', () => {
    expect(formatRelative('2026-06-07T12:00:00Z', now)).toBe('たった今')
  })

  it('formats minute / hour / day buckets', () => {
    expect(formatRelative('2026-06-07T11:30:00Z', now)).toBe('30 分前')
    expect(formatRelative('2026-06-07T09:00:00Z', now)).toBe('3 時間前')
    expect(formatRelative('2026-06-04T12:00:00Z', now)).toBe('3 日前')
  })

  it('returns dash for invalid input', () => {
    expect(formatRelative('', now)).toBe('—')
    expect(formatRelative('not-a-date', now)).toBe('—')
  })

  it('handles future timestamps as 「たった今」 instead of leaking clock skew', () => {
    expect(formatRelative('2026-06-07T13:00:00Z', now)).toBe('たった今')
  })
})
