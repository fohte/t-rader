import { Button } from '@fohte/ui/button'

import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'
import { cn } from '#lib/utils'

interface CiteBadgeProps {
  number: number
  cite: string
  className?: string
  defaultOpen?: boolean
}

export function CiteBadge({
  number,
  cite,
  className,
  defaultOpen,
}: CiteBadgeProps) {
  return (
    <Popover defaultOpen={defaultOpen}>
      <PopoverTrigger
        render={
          <Button
            variant="plain"
            aria-label={`出典 ${String(number)} を表示`}
            onClick={(e) => {
              // 親ノードのクリック/ドラッグに伝播させない (ref-chip.tsx の onOpen と同じ理由)
              e.stopPropagation()
            }}
            className={cn('size-8', className)}
          >
            <span className="font-mono text-xs">{number}</span>
          </Button>
        }
      />
      <PopoverContent
        padding="md"
        className="w-64"
        onClick={(e) => {
          e.stopPropagation()
        }}
      >
        <div className="text-sm">{cite}</div>
      </PopoverContent>
    </Popover>
  )
}
