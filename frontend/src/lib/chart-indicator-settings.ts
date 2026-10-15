import { Result } from 'neverthrow'
import { useSyncExternalStore } from 'react'

export type ChartIndicatorKind = 'sma' | 'ema' | 'bollinger'

interface ChartIndicatorBase {
  id: string
  period: number
  color: string
}

export type ChartIndicatorConfig =
  | (ChartIndicatorBase & { kind: 'sma' })
  | (ChartIndicatorBase & { kind: 'ema' })
  | (ChartIndicatorBase & { kind: 'bollinger'; deviation: number })

export const MAX_CHART_INDICATOR_PERIOD = 500
const EMPTY_CHART_INDICATORS: ChartIndicatorConfig[] = []

const STORAGE_KEY = 't-rader:chart-indicators'
const COLORS = [
  '#f97316',
  '#38bdf8',
  '#34d399',
  '#c084fc',
  '#facc15',
  '#f472b6',
] as const

const readStoredValue = Result.fromThrowable(() =>
  window.localStorage.getItem(STORAGE_KEY),
)
const writeStoredValue = Result.fromThrowable((value: string) => {
  window.localStorage.setItem(STORAGE_KEY, value)
})
const parseJson = Result.fromThrowable(
  (value: string) => JSON.parse(value) as unknown,
)

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function isValidPeriod(value: unknown): value is number {
  return (
    typeof value === 'number' &&
    Number.isInteger(value) &&
    value >= 1 &&
    value <= MAX_CHART_INDICATOR_PERIOD
  )
}

function isValidDeviation(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
}

function isChartIndicator(value: unknown): value is ChartIndicatorConfig {
  if (!isRecord(value)) return false

  const hasBaseValues =
    typeof value.id === 'string' &&
    value.id.length > 0 &&
    isValidPeriod(value.period) &&
    typeof value.color === 'string' &&
    /^#[\da-f]{6}$/i.test(value.color)

  if (!hasBaseValues) return false
  if (value.kind === 'sma' || value.kind === 'ema') return true
  return value.kind === 'bollinger' && isValidDeviation(value.deviation)
}

function parseStoredIndicators(value: string | null): ChartIndicatorConfig[] {
  if (value == null) return []

  return parseJson(value).match(
    (parsed) => (Array.isArray(parsed) ? parsed.filter(isChartIndicator) : []),
    () => [],
  )
}

function readStoredIndicators(): ChartIndicatorConfig[] {
  if (typeof window === 'undefined') return []

  return readStoredValue().match(
    (value) => parseStoredIndicators(value),
    () => [],
  )
}

let snapshot = readStoredIndicators()
const listeners = new Set<() => void>()

function getSnapshot(): ChartIndicatorConfig[] {
  return snapshot
}

function getServerSnapshot(): ChartIndicatorConfig[] {
  return EMPTY_CHART_INDICATORS
}

function emit() {
  for (const listener of listeners) listener()
}

function handleStorage(event: StorageEvent) {
  if (event.key !== STORAGE_KEY && event.key !== null) return
  snapshot = parseStoredIndicators(event.key === null ? null : event.newValue)
  emit()
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)

  if (listeners.size === 1 && typeof window !== 'undefined') {
    snapshot = readStoredIndicators()
    window.addEventListener('storage', handleStorage)
  }

  return () => {
    listeners.delete(listener)
    if (listeners.size === 0 && typeof window !== 'undefined') {
      window.removeEventListener('storage', handleStorage)
    }
  }
}

function saveSnapshot(next: ChartIndicatorConfig[]) {
  snapshot = next
  if (typeof window !== 'undefined') {
    writeStoredValue(JSON.stringify(next)).match(
      () => undefined,
      () => undefined,
    )
  }
  emit()
}

function createIndicatorId(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID()
  }
  return `${Date.now().toString()}-${Math.random().toString(36).slice(2)}`
}

export function useChartIndicators(): ChartIndicatorConfig[] {
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot)
}

export function addChartIndicator(kind: ChartIndicatorKind): string {
  const id = createIndicatorId()
  const color = COLORS[snapshot.length % COLORS.length] ?? COLORS[0]
  const indicator: ChartIndicatorConfig =
    kind === 'bollinger'
      ? { id, kind, period: 20, deviation: 2, color }
      : { id, kind, period: 25, color }

  saveSnapshot([...snapshot, indicator])
  return id
}

export function updateChartIndicatorPeriod(id: string, period: number): void {
  if (!isValidPeriod(period)) return

  const next = snapshot.map((indicator) =>
    indicator.id === id ? { ...indicator, period } : indicator,
  )
  if (next.some((indicator, index) => indicator !== snapshot[index])) {
    saveSnapshot(next)
  }
}

export function updateChartIndicatorDeviation(
  id: string,
  deviation: number,
): void {
  if (!isValidDeviation(deviation)) return

  const next = snapshot.map((indicator) =>
    indicator.id === id && indicator.kind === 'bollinger'
      ? { ...indicator, deviation }
      : indicator,
  )
  if (next.some((indicator, index) => indicator !== snapshot[index])) {
    saveSnapshot(next)
  }
}

export function removeChartIndicator(id: string): void {
  const next = snapshot.filter((indicator) => indicator.id !== id)
  if (next.length !== snapshot.length) saveSnapshot(next)
}

export function getChartIndicatorLookbackBars(
  indicators: readonly ChartIndicatorConfig[],
): number {
  return indicators.reduce(
    (maximum, indicator) => Math.max(maximum, indicator.period),
    0,
  )
}
