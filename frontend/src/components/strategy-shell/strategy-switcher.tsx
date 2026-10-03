import { Link } from '@tanstack/react-router'
import { ChevronDown } from 'lucide-react'
import { useState } from 'react'

import { useCurrentStrategyId } from '#components/strategy-shell/use-current-strategy-id'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']

export function StrategySwitcher() {
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
              className={`relative flex flex-shrink-0 cursor-pointer items-center gap-2 border px-3.5 py-1.5 font-mono text-sm ${
                active
                  ? 'border-border border-b-card bg-card text-foreground'
                  : 'border-transparent text-muted-foreground-strong hover:bg-surface-strong hover:text-foreground'
              }`}
            >
              {active && (
                <span className="absolute -inset-x-px -top-px h-0.5 bg-primary" />
              )}
              {s.name}
            </Link>
          )
        })}
      </div>
      <div className="min-w-0 flex-1 md:hidden">
        <MobileStrategyDropdown strategies={strategies} current={current} />
      </div>
    </>
  )
}

function MobileStrategyDropdown({
  strategies,
  current,
}: {
  strategies: Strategy[]
  current: Strategy | undefined
}) {
  const [open, setOpen] = useState(false)
  return (
    <div className="relative">
      <button
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => {
          setOpen((v) => !v)
        }}
        className="flex w-full items-center justify-between gap-2 border border-border bg-surface-strong px-3 py-1.5 font-mono text-sm text-foreground"
      >
        <span className="truncate">{current?.name ?? '戦略を選択'}</span>
        <ChevronDown className="size-3.5 shrink-0 text-muted-foreground" />
      </button>
      {open && (
        <ul
          role="listbox"
          className="absolute left-0 right-0 top-full z-30 mt-1 border border-border bg-bg-secondary"
        >
          {strategies.map((s) => (
            <li key={s.id}>
              <Link
                to="/strategies/$id/performance"
                params={{ id: s.id }}
                onClick={() => {
                  setOpen(false)
                }}
                className="flex items-center justify-between gap-2 border-b border-border px-3 py-2 font-mono text-sm text-muted-foreground-strong last:border-b-0 hover:bg-surface-strong hover:text-foreground"
              >
                <span className="truncate">{s.name}</span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
