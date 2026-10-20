import { describe, expect, it } from 'vitest'

import type { StrategyCandidate } from '#strategy-resolution/resolve-strategy'
import { resolveStrategy } from '#strategy-resolution/resolve-strategy'

const CANDIDATES: readonly StrategyCandidate[] = [
  { strategyId: 'north-group', name: 'NorthGroup' },
  { strategyId: 'south-group', name: 'SouthGroup' },
  { strategyId: 'amber-path', name: 'AmberPath' },
]

describe('resolveStrategy', () => {
  it('resolves when the text clearly names one strategy', () => {
    const results = [
      'use NORTHGROUP for sample-alpha',
      'North section please',
      'choose southgroup',
      'select amberpath',
      'AMBER record',
    ].map((text) => resolveStrategy(CANDIDATES, text))

    expect(results).toEqual([
      { kind: 'resolved', strategyId: 'north-group' },
      { kind: 'resolved', strategyId: 'north-group' },
      { kind: 'resolved', strategyId: 'south-group' },
      { kind: 'resolved', strategyId: 'amber-path' },
      { kind: 'resolved', strategyId: 'amber-path' },
    ])
  })

  it('reports ambiguous when the text matches multiple candidates too closely to pick one', () => {
    expect(resolveStrategy(CANDIDATES, 'Group')).toEqual({
      kind: 'ambiguous',
      candidates: [
        { strategyId: 'north-group', name: 'NorthGroup' },
        { strategyId: 'south-group', name: 'SouthGroup' },
      ],
    })
  })

  it('reports not_found with the full candidate list when nothing matches', () => {
    expect(resolveStrategy(CANDIDATES, 'sample-gamma')).toEqual({
      kind: 'not_found',
      candidates: CANDIDATES,
    })
  })

  it('reports not_found with an empty candidate list when there are no strategies', () => {
    expect(resolveStrategy([], 'use northgroup for sample-alpha')).toEqual({
      kind: 'not_found',
      candidates: [],
    })
  })

  it('resolves the sole candidate when there is nothing to disambiguate against', () => {
    const solo: readonly StrategyCandidate[] = [
      { strategyId: 'only', name: 'NorthGroup' },
    ]
    expect(resolveStrategy(solo, 'northgroup for sample-alpha')).toEqual({
      kind: 'resolved',
      strategyId: 'only',
    })
  })
})
