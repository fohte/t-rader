import { describe, expect, it } from 'vitest'

import type { StrategyCandidate } from '#strategy-resolution/resolve-strategy'
import { resolveStrategy } from '#strategy-resolution/resolve-strategy'

const CANDIDATES: readonly StrategyCandidate[] = [
  { strategyId: 'long-term', name: '長期投資' },
  { strategyId: 'mid-term', name: '中期投資' },
  { strategyId: 'swing', name: '集中スイング' },
]

describe('resolveStrategy', () => {
  it('resolves when the text clearly names one strategy', () => {
    const results = [
      'NVDAを長期投資の戦略で分析して',
      'NVDAを長期の観点で分析して',
      '中期投資の方でお願いします',
      '集中スイング戦略で見て',
      'スイングでNVDA見て',
    ].map((text) => resolveStrategy(CANDIDATES, text))

    expect(results).toEqual([
      { kind: 'resolved', strategyId: 'long-term' },
      { kind: 'resolved', strategyId: 'long-term' },
      { kind: 'resolved', strategyId: 'mid-term' },
      { kind: 'resolved', strategyId: 'swing' },
      { kind: 'resolved', strategyId: 'swing' },
    ])
  })

  it('reports ambiguous when the text matches multiple candidates too closely to pick one', () => {
    expect(resolveStrategy(CANDIDATES, '投資戦略でNVDA見て')).toEqual({
      kind: 'ambiguous',
      candidates: [
        { strategyId: 'long-term', name: '長期投資' },
        { strategyId: 'mid-term', name: '中期投資' },
      ],
    })
  })

  it('reports not_found with the full candidate list when nothing matches', () => {
    expect(resolveStrategy(CANDIDATES, 'NVDAについて教えて')).toEqual({
      kind: 'not_found',
      candidates: CANDIDATES,
    })
  })

  it('reports not_found with an empty candidate list when there are no strategies', () => {
    expect(resolveStrategy([], '長期投資でNVDA見て')).toEqual({
      kind: 'not_found',
      candidates: [],
    })
  })

  it('resolves the sole candidate when there is nothing to disambiguate against', () => {
    const solo: readonly StrategyCandidate[] = [
      { strategyId: 'only', name: '長期投資' },
    ]
    expect(resolveStrategy(solo, '長期投資でNVDA見て')).toEqual({
      kind: 'resolved',
      strategyId: 'only',
    })
  })
})
