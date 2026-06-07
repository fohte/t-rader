import { describe, expect, it } from 'vitest'

import {
  buildSnippet,
  extractRefs,
  formatRelative,
  isNewerThan,
} from '@/lib/note-utils'

describe('extractRefs', () => {
  it('returns frontmatter refs when present', () => {
    expect(
      extractRefs({
        frontmatter_json: {
          refs: ['stock:7203', 'indicator:USDJPY'],
        },
        body_md: '本文 [[stock:3436]]',
      }),
    ).toEqual(['stock:7203', 'indicator:USDJPY'])
  })

  it('falls back to body scan when frontmatter has no refs', () => {
    expect(
      extractRefs({
        frontmatter_json: {},
        body_md: '[[stock:3436]] と [[indicator:USDJPY]] を見る',
      }),
    ).toEqual(['stock:3436', 'indicator:USDJPY'])
  })

  it('deduplicates body refs', () => {
    expect(
      extractRefs({
        frontmatter_json: {},
        body_md: '[[stock:3436]] と再掲 [[stock:3436]]',
      }),
    ).toEqual(['stock:3436'])
  })

  it('filters non-string values from frontmatter refs', () => {
    expect(
      extractRefs({
        frontmatter_json: { refs: ['stock:7203', 42, null] },
        body_md: '',
      }),
    ).toEqual(['stock:7203'])
  })
})

describe('buildSnippet', () => {
  it('strips markdown and ref syntax', () => {
    expect(buildSnippet('## 見出し\n*強調* と [[stock:3436]]')).toBe(
      '見出し 強調 と 3436',
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

describe('isNewerThan', () => {
  it('treats null since as always newer', () => {
    expect(isNewerThan('2020-01-01T00:00:00Z', null)).toBe(true)
  })

  it('compares against since timestamp', () => {
    const since = new Date('2026-06-01T00:00:00Z').getTime()
    expect(isNewerThan('2026-06-02T00:00:00Z', since)).toBe(true)
    expect(isNewerThan('2026-05-30T00:00:00Z', since)).toBe(false)
  })
})
