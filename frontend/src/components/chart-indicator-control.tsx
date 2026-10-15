import { Button } from '@fohte/ui/button'
import { DialogTrigger } from '@fohte/ui/dialog'
import { useState, useSyncExternalStore } from 'react'

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '#components/ui/dialog'
import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'
import {
  type ChartIndicatorConfig,
  type ChartIndicatorKind,
  type ChartIndicatorMacdPeriodField,
  getChartIndicatorName,
  isValidChartIndicatorDeviation,
  isValidChartIndicatorMacdPeriods,
  isValidChartIndicatorPeriod,
  MAX_CHART_INDICATOR_PERIOD,
} from '#lib/chart-indicator-settings'

type ChartIndicatorControlProps = {
  indicators: ChartIndicatorConfig[]
  onAdd: (kind: ChartIndicatorKind) => void
  onUpdatePeriod: (id: string, period: number) => void
  onUpdateMacdPeriod: (
    id: string,
    field: ChartIndicatorMacdPeriodField,
    period: number,
  ) => void
  onUpdateDeviation: (id: string, deviation: number) => void
  onRemove: (id: string) => void
  open?: boolean
  defaultOpen?: boolean
  onOpenChange?: (open: boolean) => void
  layout?: 'responsive' | 'popover' | 'sheet'
}

const MOBILE_QUERY = '(max-width: 639px)'

function subscribeToViewportChange(listener: () => void): () => void {
  if (
    typeof window === 'undefined' ||
    typeof window.matchMedia !== 'function'
  ) {
    return () => {}
  }

  const mediaQuery = window.matchMedia(MOBILE_QUERY)
  mediaQuery.addEventListener('change', listener)
  return () => {
    mediaQuery.removeEventListener('change', listener)
  }
}

function getIsMobileViewport(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia(MOBILE_QUERY).matches
  )
}

function useIsMobileViewport(): boolean {
  return useSyncExternalStore(
    subscribeToViewportChange,
    getIsMobileViewport,
    () => false,
  )
}

function IndicatorPeriodInput({
  label,
  value,
  onCommit,
  isValid = isValidChartIndicatorPeriod,
  compact = false,
}: {
  label: string
  value: number
  onCommit: (period: number) => void
  isValid?: (period: number) => boolean
  compact?: boolean
}) {
  return (
    <input
      aria-label={label}
      className={`h-7 ${compact ? 'w-12' : 'w-16'} rounded border border-input bg-background px-2 text-right`}
      defaultValue={value}
      max={MAX_CHART_INDICATOR_PERIOD}
      min={1}
      onBlur={(event) => {
        const period = Number(event.currentTarget.value)
        if (!isValid(period)) {
          event.currentTarget.value = String(value)
        } else {
          onCommit(period)
        }
      }}
      onKeyDown={(event) => {
        if (event.key === 'Enter') event.currentTarget.blur()
      }}
      step={1}
      type="number"
    />
  )
}

export function ChartIndicatorControl({
  indicators,
  onAdd,
  onUpdatePeriod,
  onUpdateMacdPeriod,
  onUpdateDeviation,
  onRemove,
  open: controlledOpen,
  defaultOpen = false,
  onOpenChange,
  layout = 'responsive',
}: ChartIndicatorControlProps) {
  const [uncontrolledOpen, setUncontrolledOpen] = useState(defaultOpen)
  const isOpen = controlledOpen ?? uncontrolledOpen
  const isMobile = useIsMobileViewport()
  const useSheet = layout === 'sheet' || (layout === 'responsive' && isMobile)
  const setOpen = (nextOpen: boolean) => {
    if (controlledOpen === undefined) setUncontrolledOpen(nextOpen)
    onOpenChange?.(nextOpen)
  }

  const trigger = (
    <Button
      variant="outline"
      size="xs"
      aria-expanded={isOpen}
      aria-label={`指標 ${indicators.length.toString()}`}
    >
      指標 {indicators.length} ▾
    </Button>
  )

  const menu = (
    <ChartIndicatorMenu
      indicators={indicators}
      onAdd={onAdd}
      onUpdatePeriod={onUpdatePeriod}
      onUpdateMacdPeriod={onUpdateMacdPeriod}
      onUpdateDeviation={onUpdateDeviation}
      onRemove={onRemove}
    />
  )

  if (useSheet) {
    return (
      <Dialog open={isOpen} onOpenChange={setOpen}>
        <DialogTrigger render={trigger} />
        <DialogContent className="fixed inset-x-0 bottom-0 top-auto flex max-h-screen w-full max-w-none translate-x-0 translate-y-0 flex-col gap-0 overflow-y-auto rounded-b-none rounded-t-lg p-4">
          <DialogHeader className="pb-3">
            <DialogTitle>指標</DialogTitle>
          </DialogHeader>
          {menu}
        </DialogContent>
      </Dialog>
    )
  }

  return (
    <Popover open={isOpen} onOpenChange={setOpen}>
      <PopoverTrigger render={trigger} />
      <PopoverContent align="end" className="max-h-96 w-80 overflow-y-auto p-3">
        {menu}
      </PopoverContent>
    </Popover>
  )
}

function ChartIndicatorMenu({
  indicators,
  onAdd,
  onUpdatePeriod,
  onUpdateMacdPeriod,
  onUpdateDeviation,
  onRemove,
}: Pick<
  ChartIndicatorControlProps,
  | 'indicators'
  | 'onAdd'
  | 'onUpdatePeriod'
  | 'onUpdateMacdPeriod'
  | 'onUpdateDeviation'
  | 'onRemove'
>) {
  return (
    <div className="space-y-3 font-mono text-xs">
      <div className="space-y-2">
        <h3 className="text-muted-foreground">表示中の指標</h3>
        {indicators.length === 0 ? (
          <p className="text-muted-foreground">表示中の指標はありません</p>
        ) : (
          <ul className="space-y-2">
            {indicators.map((indicator) => {
              const name = getChartIndicatorName(indicator)
              const label =
                indicator.kind === 'macd'
                  ? `${name} ${indicator.fastPeriod.toString()} ${indicator.slowPeriod.toString()} ${indicator.signalPeriod.toString()}`
                  : `${name} ${indicator.period.toString()}`

              return (
                <li
                  key={indicator.id}
                  className="flex min-w-0 items-center gap-2 border-b border-border pb-2 last:border-b-0"
                >
                  <span
                    aria-hidden="true"
                    className="h-0.5 w-3 shrink-0"
                    style={{ backgroundColor: indicator.color }}
                  />
                  <span className="shrink-0">{name}</span>
                  {indicator.kind === 'macd' ? (
                    <div className="flex min-w-0 items-center gap-1">
                      <span
                        aria-hidden="true"
                        className="text-muted-foreground"
                      >
                        短
                      </span>
                      <IndicatorPeriodInput
                        key={`${indicator.id}-fast-${indicator.fastPeriod.toString()}`}
                        label={`${name}の短期期間`}
                        value={indicator.fastPeriod}
                        isValid={(period) =>
                          isValidChartIndicatorMacdPeriods(
                            period,
                            indicator.slowPeriod,
                            indicator.signalPeriod,
                          )
                        }
                        onCommit={(period) => {
                          onUpdateMacdPeriod(indicator.id, 'fastPeriod', period)
                        }}
                        compact
                      />
                      <span
                        aria-hidden="true"
                        className="text-muted-foreground"
                      >
                        長
                      </span>
                      <IndicatorPeriodInput
                        key={`${indicator.id}-slow-${indicator.slowPeriod.toString()}`}
                        label={`${name}の長期期間`}
                        value={indicator.slowPeriod}
                        isValid={(period) =>
                          isValidChartIndicatorMacdPeriods(
                            indicator.fastPeriod,
                            period,
                            indicator.signalPeriod,
                          )
                        }
                        onCommit={(period) => {
                          onUpdateMacdPeriod(indicator.id, 'slowPeriod', period)
                        }}
                        compact
                      />
                      <span
                        aria-hidden="true"
                        className="text-muted-foreground"
                      >
                        信
                      </span>
                      <IndicatorPeriodInput
                        key={`${indicator.id}-signal-${indicator.signalPeriod.toString()}`}
                        label={`${name}のシグナル期間`}
                        value={indicator.signalPeriod}
                        onCommit={(period) => {
                          onUpdateMacdPeriod(
                            indicator.id,
                            'signalPeriod',
                            period,
                          )
                        }}
                        compact
                      />
                    </div>
                  ) : (
                    <IndicatorPeriodInput
                      key={`${indicator.id}-period-${indicator.period.toString()}`}
                      label={`${label}の期間`}
                      value={indicator.period}
                      onCommit={(period) => {
                        onUpdatePeriod(indicator.id, period)
                      }}
                    />
                  )}
                  {indicator.kind === 'bollinger' && (
                    <>
                      <input
                        aria-label={`${label}の標準偏差`}
                        className="h-7 w-14 rounded border border-input bg-background px-2 text-right"
                        defaultValue={indicator.deviation}
                        key={`${indicator.id}-deviation`}
                        min={0.1}
                        onBlur={(event) => {
                          const deviation = Number(event.currentTarget.value)
                          if (!isValidChartIndicatorDeviation(deviation)) {
                            event.currentTarget.value = String(
                              indicator.deviation,
                            )
                          }
                        }}
                        onChange={(event) => {
                          const deviation = Number(event.currentTarget.value)
                          if (isValidChartIndicatorDeviation(deviation)) {
                            onUpdateDeviation(indicator.id, deviation)
                          }
                        }}
                        step={0.1}
                        type="number"
                      />
                      <span className="text-muted-foreground">σ</span>
                    </>
                  )}
                  <button
                    aria-label={`${label}を削除`}
                    className="ml-auto shrink-0 px-1 text-muted-foreground hover:text-foreground"
                    onClick={() => {
                      onRemove(indicator.id)
                    }}
                    type="button"
                  >
                    ×
                  </button>
                </li>
              )
            })}
          </ul>
        )}
      </div>
      <div className="space-y-2 border-t border-border pt-3">
        <h3 className="text-muted-foreground">追加する</h3>
        <div className="flex flex-wrap gap-2">
          <Button
            variant="outline"
            size="xs"
            onClick={() => {
              onAdd('sma')
            }}
            type="button"
          >
            + SMA
          </Button>
          <Button
            variant="outline"
            size="xs"
            onClick={() => {
              onAdd('ema')
            }}
            type="button"
          >
            + EMA
          </Button>
          <Button
            variant="outline"
            size="xs"
            onClick={() => {
              onAdd('bollinger')
            }}
            type="button"
          >
            + ボリンジャー
          </Button>
          <Button
            variant="outline"
            size="xs"
            onClick={() => {
              onAdd('rsi')
            }}
            type="button"
          >
            + RSI
          </Button>
          <Button
            variant="outline"
            size="xs"
            onClick={() => {
              onAdd('macd')
            }}
            type="button"
          >
            + MACD
          </Button>
        </div>
      </div>
    </div>
  )
}
