import { describe, expect, it } from 'vitest'

import { withAsOf } from '#strategy-agent/as-of'

describe('withAsOf', () => {
  it('prepends the as_of time and its non-guarantee note to the prompt', () => {
    expect(withAsOf('分析して', new Date('2026-01-02T03:04:05Z'))).toBe(
      [
        '基準時刻 (as_of): 2026-01-02T03:04:05.000Z',
        'これは実行の論理的な基準時刻であり、参照するデータがすべてこの時刻のものであることは保証されない。',
        '分析して',
      ].join('\n\n'),
    )
  })

  it('returns the prompt unchanged when as_of is absent', () => {
    expect(withAsOf('分析して', undefined)).toBe('分析して')
  })
})
