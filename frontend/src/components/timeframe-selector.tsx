import { Button } from '@fohte/ui/button'
import { useState } from 'react'

import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'
import { cn } from '#lib/utils'

/** サポートするタイムフレームの定義 */
const TIMEFRAMES = [
  { value: '1m', label: '1m' },
  { value: '5m', label: '5m' },
  { value: '15m', label: '15m' },
  { value: '1h', label: '1h' },
  { value: '4h', label: '4h' },
  { value: '1d', label: '1D' },
  { value: '1w', label: '1W' },
] as const

export type Timeframe = (typeof TIMEFRAMES)[number]['value']

const QUICK_TIMEFRAMES = TIMEFRAMES.filter(
  (timeframe) => timeframe.value === '1h' || timeframe.value === '1d',
)
const OTHER_TIMEFRAMES = TIMEFRAMES.filter(
  (timeframe) =>
    !QUICK_TIMEFRAMES.some(
      (quickTimeframe) => quickTimeframe.value === timeframe.value,
    ),
)

interface TimeframeSelectorProps {
  value: Timeframe
  onChange: (value: Timeframe) => void
  className?: string
  variant?: 'default' | 'compact'
  moreTimeframesDefaultOpen?: boolean
}

export function TimeframeSelector({
  value,
  onChange,
  className,
  variant = 'default',
  moreTimeframesDefaultOpen = false,
}: TimeframeSelectorProps) {
  const [moreTimeframesOpen, setMoreTimeframesOpen] = useState(
    moreTimeframesDefaultOpen,
  )
  const timeframes = variant === 'compact' ? QUICK_TIMEFRAMES : TIMEFRAMES
  const hasOtherTimeframeSelected = OTHER_TIMEFRAMES.some(
    (timeframe) => timeframe.value === value,
  )

  return (
    <div className={cn('inline-flex gap-1', className)}>
      {timeframes.map((timeframe) => (
        <TimeframeButton
          key={timeframe.value}
          timeframe={timeframe}
          isSelected={timeframe.value === value}
          onChange={onChange}
        />
      ))}
      {variant === 'compact' && (
        <Popover open={moreTimeframesOpen} onOpenChange={setMoreTimeframesOpen}>
          <PopoverTrigger
            render={
              <Button
                variant={hasOtherTimeframeSelected ? 'default' : 'outline'}
                size="xs"
                aria-label="その他の時間足"
                title="その他の時間足"
              >
                …
              </Button>
            }
          />
          <PopoverContent align="end" className="w-24 p-1">
            <div
              role="group"
              aria-label="その他の時間足"
              className="flex flex-col gap-1"
            >
              {OTHER_TIMEFRAMES.map((timeframe) => (
                <TimeframeButton
                  key={timeframe.value}
                  timeframe={timeframe}
                  isSelected={timeframe.value === value}
                  onChange={(nextValue) => {
                    onChange(nextValue)
                    setMoreTimeframesOpen(false)
                  }}
                  className="w-full justify-start"
                />
              ))}
            </div>
          </PopoverContent>
        </Popover>
      )}
    </div>
  )
}

function TimeframeButton({
  timeframe,
  isSelected,
  onChange,
  className,
}: {
  timeframe: (typeof TIMEFRAMES)[number]
  isSelected: boolean
  onChange: (value: Timeframe) => void
  className?: string
}) {
  return (
    <Button
      variant={isSelected ? 'default' : 'outline'}
      size="xs"
      onClick={() => {
        onChange(timeframe.value)
      }}
      aria-pressed={isSelected}
      className={className}
    >
      {timeframe.label}
    </Button>
  )
}
