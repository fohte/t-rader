import { Fragment } from 'react'

import { Button } from '@/components/ui/button'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'

/** サポートするタイムフレームの定義 */
export const TIMEFRAMES = [
  { value: '5m', label: '5m', enabled: false },
  { value: '15m', label: '15m', enabled: false },
  { value: '1h', label: '1h', enabled: false },
  { value: '4h', label: '4h', enabled: false },
  { value: '1d', label: '1D', enabled: true },
  { value: '1w', label: '1W', enabled: false },
] as const

export type Timeframe = (typeof TIMEFRAMES)[number]['value']

/** 有効なタイムフレームのみを抽出 */
export const ENABLED_TIMEFRAMES = TIMEFRAMES.filter((tf) => tf.enabled).map(
  (tf) => tf.value,
)

interface TimeframeSelectorProps {
  value: Timeframe
  onChange: (value: Timeframe) => void
  className?: string
}

export function TimeframeSelector({
  value,
  onChange,
  className,
}: TimeframeSelectorProps) {
  return (
    <TooltipProvider>
      <div className={cn('inline-flex gap-1', className)}>
        {TIMEFRAMES.map((tf) => {
          const isSelected = tf.value === value
          const isDisabled = !tf.enabled

          const button = (
            <Button
              variant={isSelected ? 'default' : 'outline'}
              size="xs"
              disabled={isDisabled}
              onClick={() => onChange(tf.value)}
              aria-pressed={isSelected}
            >
              {tf.label}
            </Button>
          )

          if (isDisabled) {
            return (
              <Tooltip key={tf.value}>
                <TooltipTrigger asChild>
                  {/* disabled なボタンはイベントを受け取れないため span でラップ */}
                  <span>{button}</span>
                </TooltipTrigger>
                <TooltipContent>近日対応予定</TooltipContent>
              </Tooltip>
            )
          }

          return <Fragment key={tf.value}>{button}</Fragment>
        })}
      </div>
    </TooltipProvider>
  )
}
