import { XIcon } from 'lucide-react'

import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

interface ChartMarketDepthPanelProps {
  instrumentId: string
  isOpen: boolean
  onToggle: () => void
  className?: string
}

export function ChartMarketDepthPanel({
  instrumentId,
  isOpen,
  onToggle,
  className,
}: ChartMarketDepthPanelProps) {
  if (!isOpen) {
    return null
  }

  return (
    <div
      className={cn(
        'flex w-72 shrink-0 flex-col rounded-md border bg-card p-4',
        className,
      )}
    >
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold">板情報・歩み値</h2>
        <Button
          variant="ghost"
          size="icon-xs"
          onClick={onToggle}
          aria-label="パネルを閉じる"
        >
          <XIcon />
        </Button>
      </div>
      <div className="mt-4 flex flex-1 items-center justify-center">
        <p className="text-sm text-muted-foreground">{instrumentId} - 開発中</p>
      </div>
    </div>
  )
}
