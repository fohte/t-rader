import { Button } from '@fohte/ui/button'
import { Link } from '@tanstack/react-router'
import { ChevronDown } from 'lucide-react'
import { useState } from 'react'

import { useCurrentStrategyId } from '#components/strategy-shell/use-current-strategy-id'
import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'
import { cn } from '#lib/utils'

type Strategy = components['schemas']['Strategy']

export function StrategySwitcher({
  mobileOpen,
}: {
  mobileOpen?: boolean
} = {}) {
  const currentId = useCurrentStrategyId()
  const { data: strategies = [] } = $api.useQuery('get', '/api/strategies')
  const current = strategies.find((s) => s.id === currentId)

  return (
    <>
      <div className="hidden min-w-0 flex-1 items-stretch gap-0.5 overflow-x-auto [scrollbar-width:none] md:flex [&::-webkit-scrollbar]:hidden">
        {strategies.map((s) => {
          const active = currentId === s.id
          return (
            <Link
              key={s.id}
              to="/strategies/$id/performance"
              params={{ id: s.id }}
              className={cn(
                'relative flex flex-shrink-0 cursor-pointer items-center gap-2 border px-3.5 py-1.5 font-mono text-sm',
                active
                  ? 'border-border border-b-card bg-card text-foreground'
                  : 'border-transparent text-muted-foreground-strong hover:bg-surface-strong hover:text-foreground',
              )}
            >
              {active && (
                <span className="absolute -inset-x-px -top-px h-0.5 bg-foreground" />
              )}
              {s.name}
            </Link>
          )
        })}
      </div>
      <div className="min-w-0 flex-1 md:hidden">
        <MobileStrategyDropdown
          strategies={strategies}
          current={current}
          open={mobileOpen}
        />
      </div>
    </>
  )
}

function MobileStrategyDropdown({
  strategies,
  current,
  open: controlledOpen,
}: {
  strategies: Strategy[]
  current: Strategy | undefined
  open?: boolean
}) {
  const [uncontrolledOpen, setUncontrolledOpen] = useState(false)
  const open = controlledOpen ?? uncontrolledOpen

  function handleOpenChange(nextOpen: boolean) {
    if (controlledOpen == null) setUncontrolledOpen(nextOpen)
  }

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger
        render={
          <Button
            variant="secondary"
            aria-label="戦略を切り替える"
            className="w-full min-w-0 justify-between"
          >
            <span className="truncate">{current?.name ?? '戦略を選択'}</span>
            <ChevronDown aria-hidden="true" />
          </Button>
        }
      />
      <PopoverContent
        align="start"
        padding="none"
        className="max-h-64 w-56 overflow-y-auto"
      >
        <nav aria-label="戦略切り替え" className="flex flex-col">
          {strategies.map((s) => (
            <Link
              key={s.id}
              to="/strategies/$id/performance"
              params={{ id: s.id }}
              aria-current={current?.id === s.id ? 'page' : undefined}
              onClick={() => {
                handleOpenChange(false)
              }}
              className="flex items-center justify-between gap-2 border-b border-border px-3 py-2 text-sm text-muted-foreground-strong last:border-b-0 hover:bg-surface-strong hover:text-foreground"
            >
              <span className="truncate">{s.name}</span>
            </Link>
          ))}
        </nav>
      </PopoverContent>
    </Popover>
  )
}
