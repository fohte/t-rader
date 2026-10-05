import { describe, expect, it } from 'vitest'

import { buildSnippet, formatRelative } from '#lib/note-utils'

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
