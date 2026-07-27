import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { useLastVisited } from '#components/strategy-home/use-last-visited'

describe('useLastVisited', () => {
  beforeEach(() => {
    window.localStorage.clear()
  })
  afterEach(() => {
    window.localStorage.clear()
  })

  it('returns null on first visit and writes current time', () => {
    const { result } = renderHook(() => useLastVisited('s1'))
    expect(result.current).toBeNull()
    const raw = window.localStorage.getItem('t-rader:lastVisited:s1')
    expect(raw).not.toBeNull()
  })

  it('returns previous timestamp on re-mount and overwrites with now', () => {
    window.localStorage.setItem('t-rader:lastVisited:s1', '1700000000000')
    const { result } = renderHook(() => useLastVisited('s1'))
    expect(result.current).toBe(1700000000000)
    const raw = window.localStorage.getItem('t-rader:lastVisited:s1')
    expect(Number(raw)).toBeGreaterThan(1700000000000)
  })

  it('switches snapshot and writes when strategyId changes without unmount', () => {
    window.localStorage.setItem('t-rader:lastVisited:s1', '1700000000000')
    window.localStorage.setItem('t-rader:lastVisited:s2', '1700000005000')
    const { result, rerender } = renderHook(
      ({ id }: { id: string }) => useLastVisited(id),
      { initialProps: { id: 's1' } },
    )
    expect(result.current).toBe(1700000000000)

    act(() => {
      rerender({ id: 's2' })
    })
    expect(result.current).toBe(1700000005000)
    const raw = window.localStorage.getItem('t-rader:lastVisited:s2')
    expect(Number(raw)).toBeGreaterThan(1700000005000)
  })
})
