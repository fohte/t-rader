import { Result } from 'neverthrow'
import { useSyncExternalStore } from 'react'

export type ChartIndicatorKind = 'sma' | 'ema' | 'bollinger' | 'rsi' | 'macd'

export type ChartIndicatorMacdPeriodField =
  'fastPeriod' | 'slowPeriod' | 'signalPeriod'

interface ChartIndicatorBase {
  id: string
  color: string
}

interface ChartIndicatorWithPeriod extends ChartIndicatorBase {
  period: number
}

export type ChartIndicatorConfig =
  | (ChartIndicatorWithPeriod & { kind: 'sma' })
  | (ChartIndicatorWithPeriod & { kind: 'ema' })
  | (ChartIndicatorWithPeriod & { kind: 'bollinger'; deviation: number })
  | (ChartIndicatorWithPeriod & { kind: 'rsi' })
  | (ChartIndicatorBase & {
      kind: 'macd'
      fastPeriod: number
      slowPeriod: number
      signalPeriod: number
    })

export const MAX_CHART_INDICATOR_PERIOD = 500
export const CHART_INDICATOR_PANE_HEIGHT = 80
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

export function isValidChartIndicatorPeriod(value: unknown): value is number {
  return (
    typeof value === 'number' &&
    Number.isInteger(value) &&
    value >= 1 &&
    value <= MAX_CHART_INDICATOR_PERIOD
  )
}

export function isValidChartIndicatorMacdPeriods(
  fastPeriod: unknown,
  slowPeriod: unknown,
  signalPeriod: unknown,
): boolean {
  return (
    isValidChartIndicatorPeriod(fastPeriod) &&
    isValidChartIndicatorPeriod(slowPeriod) &&
    isValidChartIndicatorPeriod(signalPeriod) &&
    fastPeriod < slowPeriod
  )
}

export function isValidChartIndicatorDeviation(
  value: unknown,
): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
}

export function getChartIndicatorName(indicator: ChartIndicatorConfig): string {
  switch (indicator.kind) {
    case 'sma':
      return 'SMA'
    case 'ema':
      return 'EMA'
    case 'bollinger':
      return 'ボリンジャー'
    case 'rsi':
      return 'RSI'
    case 'macd':
      return 'MACD'
  }
}

export function getChartIndicatorLabel(
  indicator: ChartIndicatorConfig,
): string {
  const name = getChartIndicatorName(indicator)
  return indicator.kind === 'macd'
    ? `${name} ${indicator.fastPeriod.toString()} ${indicator.slowPeriod.toString()} ${indicator.signalPeriod.toString()}`
    : `${name} ${indicator.period.toString()}`
}

export function getChartIndicatorPaneIndexes(
  indicators: readonly ChartIndicatorConfig[],
): number[] {
  let nextLowerPaneIndex = 0
  return indicators.map((indicator) =>
    indicator.kind === 'rsi' || indicator.kind === 'macd'
      ? ++nextLowerPaneIndex
      : 0,
  )
}

function isChartIndicator(value: unknown): value is ChartIndicatorConfig {
  if (!isRecord(value)) return false

  const hasBaseValues =
    typeof value.id === 'string' &&
    value.id.length > 0 &&
    typeof value.color === 'string' &&
    /^#[\da-f]{6}$/i.test(value.color)

  if (!hasBaseValues) return false

  switch (value.kind) {
    case 'sma':
    case 'ema':
    case 'rsi':
      return isValidChartIndicatorPeriod(value.period)
    case 'bollinger':
      return (
        isValidChartIndicatorPeriod(value.period) &&
        isValidChartIndicatorDeviation(value.deviation)
      )
    case 'macd':
      return isValidChartIndicatorMacdPeriods(
        value.fastPeriod,
        value.slowPeriod,
        value.signalPeriod,
      )
    default:
      return false
  }
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
      : kind === 'rsi'
        ? { id, kind, period: 14, color }
        : kind === 'macd'
          ? {
              id,
              kind,
              fastPeriod: 12,
              slowPeriod: 26,
              signalPeriod: 9,
              color,
            }
          : { id, kind, period: 25, color }

  saveSnapshot([...snapshot, indicator])
  return id
}

export function updateChartIndicatorPeriod(id: string, period: number): void {
  if (!isValidChartIndicatorPeriod(period)) return

  const next = snapshot.map((indicator) =>
    indicator.id === id && indicator.kind !== 'macd'
      ? { ...indicator, period }
      : indicator,
  )
  if (next.some((indicator, index) => indicator !== snapshot[index])) {
    saveSnapshot(next)
  }
}

export function updateChartIndicatorDeviation(
  id: string,
  deviation: number,
): void {
  if (!isValidChartIndicatorDeviation(deviation)) return

  const next = snapshot.map((indicator) =>
    indicator.id === id && indicator.kind === 'bollinger'
      ? { ...indicator, deviation }
      : indicator,
  )
  if (next.some((indicator, index) => indicator !== snapshot[index])) {
    saveSnapshot(next)
  }
}

export function updateChartIndicatorMacdPeriod(
  id: string,
  field: ChartIndicatorMacdPeriodField,
  period: number,
): void {
  if (!isValidChartIndicatorPeriod(period)) return

  const indicator = snapshot.find(
    (candidate) => candidate.id === id && candidate.kind === 'macd',
  )
  if (indicator == null || indicator.kind !== 'macd') return

  const updated = { ...indicator, [field]: period }
  if (
    !isValidChartIndicatorMacdPeriods(
      updated.fastPeriod,
      updated.slowPeriod,
      updated.signalPeriod,
    )
  ) {
    return
  }

  saveSnapshot(
    snapshot.map((candidate) =>
      candidate.id === id && candidate.kind === 'macd' ? updated : candidate,
    ),
  )
}

export function removeChartIndicator(id: string): void {
  const next = snapshot.filter((indicator) => indicator.id !== id)
  if (next.length !== snapshot.length) saveSnapshot(next)
}

export function getChartIndicatorLookbackBars(
  indicators: readonly ChartIndicatorConfig[],
): number {
  return indicators.reduce((maximum, indicator) => {
    const lookback =
      indicator.kind === 'rsi'
        ? indicator.period + 1
        : indicator.kind === 'macd'
          ? Math.max(indicator.fastPeriod, indicator.slowPeriod) +
            indicator.signalPeriod -
            1
          : indicator.period
    return Math.max(maximum, lookback)
  }, 0)
}

export function getChartIndicatorPaneCount(
  indicators: readonly ChartIndicatorConfig[],
): number {
  return getChartIndicatorPaneIndexes(indicators).reduce(
    (maximum, paneIndex) => Math.max(maximum, paneIndex),
    0,
  )
}
