import { afterEach, describe, expect, it } from 'vitest'

import {
  readLastUsedStrategyId,
  resolveDefaultStrategyId,
  saveLastUsedStrategyId,
} from '#components/strategy-shell/floating-chat-strategy'

afterEach(() => {
  window.localStorage.clear()
})

describe('floating chat strategy selection', () => {
  it('uses the current strategy before the last sent strategy', () => {
    expect(
      resolveDefaultStrategyId({
        currentStrategyId: 'strategy-current',
        lastUsedStrategyId: 'strategy-previous',
        availableStrategyIds: ['strategy-current', 'strategy-previous'],
      }),
    ).toBe('strategy-current')
  })

  it('uses the last sent strategy outside a strategy page', () => {
    expect(
      resolveDefaultStrategyId({
        currentStrategyId: null,
        lastUsedStrategyId: 'strategy-previous',
        availableStrategyIds: ['strategy-previous'],
      }),
    ).toBe('strategy-previous')
  })

  it('leaves the strategy unselected when there is no saved available strategy', () => {
    expect(
      resolveDefaultStrategyId({
        currentStrategyId: null,
        lastUsedStrategyId: 'strategy-deleted',
        availableStrategyIds: ['strategy-current'],
      }),
    ).toBeNull()
  })

  it('remembers the last strategy used for sending', () => {
    saveLastUsedStrategyId('strategy-previous')

    expect(readLastUsedStrategyId()).toBe('strategy-previous')
  })
})
