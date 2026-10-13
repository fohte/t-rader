const LAST_USED_STRATEGY_KEY = 't-rader:floating-chat:last-used-strategy'

export function readLastUsedStrategyId(): string | null {
  if (typeof window === 'undefined') return null
  return window.localStorage.getItem(LAST_USED_STRATEGY_KEY)
}

export function saveLastUsedStrategyId(strategyId: string): void {
  if (typeof window === 'undefined') return
  window.localStorage.setItem(LAST_USED_STRATEGY_KEY, strategyId)
}

export function resolveDefaultStrategyId({
  currentStrategyId,
  lastUsedStrategyId,
  availableStrategyIds,
}: {
  currentStrategyId: string | null
  lastUsedStrategyId: string | null
  availableStrategyIds: readonly string[]
}): string | null {
  if (currentStrategyId != null) return currentStrategyId
  if (
    lastUsedStrategyId != null &&
    availableStrategyIds.includes(lastUsedStrategyId)
  ) {
    return lastUsedStrategyId
  }
  return null
}
