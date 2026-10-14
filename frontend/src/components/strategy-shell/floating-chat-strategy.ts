import { Result } from 'neverthrow'

const LAST_USED_STRATEGY_KEY = 't-rader:floating-chat:last-used-strategy'

const readStoredStrategyId = Result.fromThrowable(() =>
  window.localStorage.getItem(LAST_USED_STRATEGY_KEY),
)
const writeStoredStrategyId = Result.fromThrowable((strategyId: string) => {
  window.localStorage.setItem(LAST_USED_STRATEGY_KEY, strategyId)
})

export function readLastUsedStrategyId(): string | null {
  if (typeof window === 'undefined') return null
  return readStoredStrategyId().match(
    (strategyId) => strategyId,
    () => null,
  )
}

export function saveLastUsedStrategyId(strategyId: string): void {
  if (typeof window === 'undefined') return
  writeStoredStrategyId(strategyId).match(
    () => undefined,
    () => undefined,
  )
}

export function resolveDefaultStrategyId({
  currentStrategyId,
  contextStrategyId,
  lastUsedStrategyId,
  availableStrategyIds,
}: {
  currentStrategyId: string | null
  contextStrategyId: string | null
  lastUsedStrategyId: string | null
  availableStrategyIds: readonly string[]
}): string | null {
  if (currentStrategyId != null) return currentStrategyId
  if (contextStrategyId != null) return contextStrategyId
  if (
    lastUsedStrategyId != null &&
    availableStrategyIds.includes(lastUsedStrategyId)
  ) {
    return lastUsedStrategyId
  }
  return null
}
