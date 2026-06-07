import { RefChip } from '@/components/strategy-shell/ref-chip'
import type { components } from '@/lib/api/schema.gen'

type StrategyInterest = components['schemas']['StrategyInterest']

interface InterestTreeProps {
  interests: StrategyInterest[]
}

export function InterestTree({ interests }: InterestTreeProps) {
  const seeds = interests.filter((i) => i.role === 'seed')
  const derived = interests.filter((i) => i.role === 'derived')

  return (
    <section className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)]">
      <div className="flex items-baseline justify-between border-b border-[color:var(--color-hairline)] px-3.5 py-2">
        <h3 className="font-mono text-[12px] font-bold uppercase tracking-wider text-[color:var(--color-text-primary)]">
          関心ツリー
        </h3>
        <span className="font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
          LLM 派生含む
        </span>
      </div>
      <div className="space-y-3 px-3.5 py-3">
        {seeds.length === 0 && derived.length === 0 ? (
          <div className="font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
            —
          </div>
        ) : (
          <>
            {seeds.length > 0 && (
              <Section title="seed">
                {seeds.map((i) => (
                  <li key={i.ref_kind + ':' + i.ref_id}>
                    <RefChip token={`${i.ref_kind}:${i.ref_id}`} />
                  </li>
                ))}
              </Section>
            )}
            {derived.length > 0 && (
              <Section title="derived">
                {derived.map((i) => (
                  <li
                    key={i.ref_kind + ':' + i.ref_id}
                    className="flex items-center gap-2"
                  >
                    <RefChip token={`${i.ref_kind}:${i.ref_id}`} />
                    <span className="font-mono text-[10px] uppercase tracking-wider text-[color:var(--color-accent-strategy)]">
                      LLM 派生
                    </span>
                  </li>
                ))}
              </Section>
            )}
          </>
        )}
      </div>
    </section>
  )
}

function Section({
  title,
  children,
}: {
  title: string
  children: React.ReactNode
}) {
  return (
    <div>
      <div className="mb-1.5 font-mono text-[10px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
        {title}
      </div>
      <ul className="flex flex-col gap-1.5">{children}</ul>
    </div>
  )
}
