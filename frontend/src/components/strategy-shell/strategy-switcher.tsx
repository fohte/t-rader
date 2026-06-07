import { Link } from '@tanstack/react-router'
import { ChevronDown } from 'lucide-react'
import { useMemo, useState } from 'react'

import { UnreadBadge } from '@/components/strategy-shell/unread-badge'
import { useCurrentStrategyId } from '@/components/strategy-shell/use-current-strategy-id'
import { $api } from '@/lib/api/client'
import type { components } from '@/lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']

export function StrategySwitcher() {
  const currentId = useCurrentStrategyId()
  const { data: strategies = [] } = $api.useQuery('get', '/api/strategies')
  const { data: unreadNotes = [] } = $api.useQuery('get', '/api/notes', {
    params: { query: { status: 'unread' } },
  })

  const unreadByStrategy = useMemo(() => {
    const m = new Map<string, number>()
    for (const n of unreadNotes) {
      m.set(n.strategy_id, (m.get(n.strategy_id) ?? 0) + 1)
    }
    return m
  }, [unreadNotes])

  const current = strategies.find((s) => s.id === currentId)

  return (
    <>
      <div className="hidden min-w-0 flex-1 items-stretch gap-0.5 overflow-x-auto [scrollbar-width:none] md:flex [&::-webkit-scrollbar]:hidden">
        {strategies.map((s) => {
          const active = currentId === s.id
          const unread = unreadByStrategy.get(s.id) ?? 0
          return (
            <Link
              key={s.id}
              to="/strategies/$id"
              params={{ id: s.id }}
              className={`relative flex flex-shrink-0 cursor-pointer items-center gap-2 border px-3.5 py-1.5 font-mono text-[13px] ${
                active
                  ? 'border-[color:var(--color-border-strategy)] border-b-[color:var(--panel)] bg-[color:var(--panel)] text-[color:var(--color-text-primary)]'
                  : 'border-transparent text-[color:var(--color-text-secondary)] hover:bg-[color:var(--panel-inset)] hover:text-[color:var(--color-text-primary)]'
              }`}
            >
              {active && (
                <span className="absolute inset-x-[-1px] top-[-1px] h-0.5 bg-[color:var(--color-accent-strategy)]" />
              )}
              {s.name}
              <UnreadBadge count={unread} />
            </Link>
          )
        })}
      </div>
      <div className="min-w-0 flex-1 md:hidden">
        <MobileStrategyDropdown
          strategies={strategies}
          current={current}
          unreadByStrategy={unreadByStrategy}
        />
      </div>
    </>
  )
}

function MobileStrategyDropdown({
  strategies,
  current,
  unreadByStrategy,
}: {
  strategies: Strategy[]
  current: Strategy | undefined
  unreadByStrategy: Map<string, number>
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
        className="flex w-full items-center justify-between gap-2 border border-[color:var(--color-border-strategy)] bg-[color:var(--panel-inset)] px-3 py-1.5 font-mono text-[13px] text-[color:var(--color-text-primary)]"
      >
        <span className="truncate">{current?.name ?? '戦略を選択'}</span>
        <ChevronDown className="size-3.5 shrink-0 text-[color:var(--color-text-tertiary)]" />
      </button>
      {open && (
        <ul
          role="listbox"
          className="absolute left-0 right-0 top-full z-30 mt-1 border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-secondary)]"
        >
          {strategies.map((s) => (
            <li key={s.id}>
              <Link
                to="/strategies/$id"
                params={{ id: s.id }}
                onClick={() => {
                  setOpen(false)
                }}
                className="flex items-center justify-between gap-2 border-b border-[color:var(--color-hairline)] px-3 py-2 font-mono text-[13px] text-[color:var(--color-text-secondary)] last:border-b-0 hover:bg-[color:var(--panel-inset)] hover:text-[color:var(--color-text-primary)]"
              >
                <span className="truncate">{s.name}</span>
                <UnreadBadge count={unreadByStrategy.get(s.id) ?? 0} />
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
