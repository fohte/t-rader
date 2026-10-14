import { afterEach, describe, expect, it } from 'vitest'

import {
  readLastUsedStrategyId,
  resolveDefaultStrategyId,
  saveLastUsedStrategyId,
} from '#components/strategy-shell/floating-chat-strategy'

afterEach(() => {
  window.localStorage.clear()
})

function readStorageFailureOutput() {
  const descriptor = Object.getOwnPropertyDescriptor(window, 'localStorage')
  Object.defineProperty(window, 'localStorage', {
    configurable: true,
    get() {
      throw new Error('storage unavailable')
    },
  })

  let storedStrategyId: string | null = 'unexpected'
  let writeCompleted = false
  try {
    storedStrategyId = readLastUsedStrategyId()
    saveLastUsedStrategyId('strategy-previous')
    writeCompleted = true
  } finally {
    if (descriptor == null) {
      Reflect.deleteProperty(window, 'localStorage')
    } else {
      Object.defineProperty(window, 'localStorage', descriptor)
    }
  }
  return { storedStrategyId, writeCompleted }
}

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

  it('keeps storage failures from interrupting chat use', () => {
    expect(readStorageFailureOutput()).toEqual({
      storedStrategyId: null,
      writeCompleted: true,
    })
  })
})
