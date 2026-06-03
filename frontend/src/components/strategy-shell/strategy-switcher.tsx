import { Link } from '@tanstack/react-router'
import { ChevronDown } from 'lucide-react'
import { useState } from 'react'

import { useCurrentStrategyId } from '@/components/strategy-shell/use-current-strategy-id'
import { STRATEGIES_MOCK, type Strategy } from '@/lib/strategy-mock'

interface StrategySwitcherProps {
  strategies?: Strategy[]
}

export function StrategySwitcher({
  strategies = STRATEGIES_MOCK,
}: StrategySwitcherProps) {
  const currentId = useCurrentStrategyId()
  const current = strategies.find((s) => s.id === currentId)

  return (
    <>
      {/* desktop: tabs */}
      <div className="hidden min-w-0 flex-1 items-stretch gap-0.5 overflow-x-auto [scrollbar-width:none] md:flex [&::-webkit-scrollbar]:hidden">
        {strategies.map((s) => {
          const active = currentId === s.id
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
              {s.unread > 0 && (
                <span className="inline-grid h-4 min-w-[16px] place-items-center bg-[color:var(--color-accent-strategy)] px-1 font-mono text-[10px] text-white">
                  {s.unread}
                </span>
              )}
            </Link>
          )
        })}
      </div>
      {/* mobile: dropdown */}
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
        onClick={() => {
          setOpen((v) => !v)
        }}
        className="flex w-full items-center justify-between gap-2 border border-[color:var(--color-border-strategy)] bg-[color:var(--panel-inset)] px-3 py-1.5 font-mono text-[13px] text-[color:var(--color-text-primary)]"
      >
        <span className="truncate">{current?.name ?? '戦略を選択'}</span>
        <ChevronDown className="size-3.5 shrink-0 text-[color:var(--color-text-tertiary)]" />
      </button>
      {open && (
        <ul className="absolute left-0 right-0 top-full z-30 mt-1 border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-secondary)]">
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
                {s.unread > 0 && (
                  <span className="inline-grid h-4 min-w-[16px] place-items-center bg-[color:var(--color-accent-strategy)] px-1 text-[10px] text-white">
                    {s.unread}
                  </span>
                )}
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
