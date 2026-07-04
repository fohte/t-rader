import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import { CreateInterestDialog } from '@/components/strategy-home/create-interest-dialog'
import { RefChip } from '@/components/strategy-shell/ref-chip'
import { Skeleton } from '@/components/ui/skeleton'
import { $api } from '@/lib/api/client'
import type { components } from '@/lib/api/schema.gen'

type StrategyInterest = components['schemas']['StrategyInterest']

export const ORIGIN_LABEL: Record<string, string> = {
  human: '人力',
  llm: 'LLM',
}

interface InterestTreeProps {
  strategyId: string
}

export function InterestTree({ strategyId }: InterestTreeProps) {
  const queryClient = useQueryClient()
  const listQueryOptions = $api.queryOptions(
    'get',
    '/api/strategies/{id}/interests',
    { params: { path: { id: strategyId } } },
  )
  const { data, isPending, isError } = useQuery(listQueryOptions)
  const interests = data ?? []

  const deleteMutation = $api.useMutation(
    'delete',
    '/api/strategies/{id}/interests/{ref_kind}/{ref_id}',
  )

  const [dialogOpen, setDialogOpen] = useState(false)
  const [listError, setListError] = useState<string | null>(null)

  const seeds = interests.filter((i) => i.role === 'seed')
  const derived = interests.filter((i) => i.role === 'derived')

  function handleDelete(interest: StrategyInterest) {
    if (deleteMutation.isPending) return
    if (
      !window.confirm(
        `関心 ${interest.ref_kind}:${interest.ref_id} を削除しますか?`,
      )
    ) {
      return
    }
    deleteMutation.mutate(
      {
        params: {
          path: {
            id: strategyId,
            ref_kind: interest.ref_kind,
            ref_id: interest.ref_id,
          },
        },
      },
      {
        onSuccess: () => {
          void queryClient.invalidateQueries({
            queryKey: listQueryOptions.queryKey,
          })
          setListError(null)
        },
        onError: () => {
          setListError('関心の削除に失敗しました')
        },
      },
    )
  }

  return (
    <section className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)]">
      <div className="flex items-baseline justify-between border-b border-[color:var(--color-hairline)] px-3.5 py-2">
        <h3 className="font-mono text-[12px] font-bold uppercase tracking-wider text-[color:var(--color-text-primary)]">
          関心ツリー
        </h3>
        <button
          type="button"
          onClick={() => {
            setDialogOpen(true)
          }}
          className="font-mono text-[11px] text-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-accent-strategy)]"
        >
          + 追加
        </button>
      </div>
      <div className="space-y-3 px-3.5 py-3">
        {isPending ? (
          <Skeleton className="h-16 w-full" />
        ) : isError ? (
          <p
            data-testid="interest-list-error"
            className="font-mono text-[12px] text-[color:var(--color-accent-strategy)]"
          >
            関心一覧の取得に失敗しました
          </p>
        ) : (
          <>
            {listError != null && (
              <p
                data-testid="interest-list-error"
                className="font-mono text-[11px] text-[color:var(--color-accent-strategy)]"
              >
                {listError}
              </p>
            )}
            {seeds.length === 0 && derived.length === 0 ? (
              <div className="font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
                —
              </div>
            ) : (
              <>
                {seeds.length > 0 && (
                  <Section title="seed">
                    {seeds.map((i) => (
                      <InterestRow
                        key={i.ref_kind + ':' + i.ref_id}
                        interest={i}
                        onDelete={handleDelete}
                      />
                    ))}
                  </Section>
                )}
                {derived.length > 0 && (
                  <Section title="derived">
                    {derived.map((i) => (
                      <InterestRow
                        key={i.ref_kind + ':' + i.ref_id}
                        interest={i}
                        onDelete={handleDelete}
                      />
                    ))}
                  </Section>
                )}
              </>
            )}
          </>
        )}
      </div>
      <CreateInterestDialog
        strategyId={strategyId}
        open={dialogOpen}
        onOpenChange={setDialogOpen}
      />
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

function InterestRow({
  interest,
  onDelete,
}: {
  interest: StrategyInterest
  onDelete: (interest: StrategyInterest) => void
}) {
  return (
    <li
      data-testid={`interest-row-${interest.ref_kind}-${interest.ref_id}`}
      className="flex items-center gap-2"
    >
      <RefChip token={`${interest.ref_kind}:${interest.ref_id}`} />
      <span className="font-mono text-[10px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
        {ORIGIN_LABEL[interest.origin] ?? interest.origin}
      </span>
      <button
        type="button"
        onClick={() => {
          onDelete(interest)
        }}
        aria-label={`関心 ${interest.ref_kind}:${interest.ref_id} を削除`}
        className="ml-auto font-mono text-[10px] text-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-accent-strategy)]"
      >
        削除
      </button>
    </li>
  )
}
