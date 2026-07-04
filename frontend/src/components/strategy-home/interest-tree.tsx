import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import { RefChip } from '@/components/strategy-shell/ref-chip'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Skeleton } from '@/components/ui/skeleton'
import { $api } from '@/lib/api/client'
import type { components } from '@/lib/api/schema.gen'
import { REF_KIND_JP, type RefKind } from '@/lib/strategy-mock'

type StrategyInterest = components['schemas']['StrategyInterest']

const REF_KINDS: RefKind[] = ['stock', 'indicator', 'sector', 'theme']
const ROLES = ['seed', 'derived'] as const
const ORIGINS = ['human', 'llm'] as const
const ORIGIN_LABEL: Record<string, string> = { human: '人力', llm: 'LLM' }

type Role = (typeof ROLES)[number]
type Origin = (typeof ORIGINS)[number]

function parseRefKind(value: string): RefKind {
  switch (value) {
    case 'stock':
    case 'indicator':
    case 'sector':
    case 'theme':
      return value
    default:
      return 'stock'
  }
}

function parseRole(value: string): Role {
  return value === 'derived' ? 'derived' : 'seed'
}

function parseOrigin(value: string): Origin {
  return value === 'llm' ? 'llm' : 'human'
}

interface InterestTreeProps {
  strategyId: string
}

interface FormState {
  refKind: RefKind
  refId: string
  role: Role
  origin: Origin
}

const EMPTY_FORM: FormState = {
  refKind: 'stock',
  refId: '',
  role: 'seed',
  origin: 'human',
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

  const createMutation = $api.useMutation(
    'post',
    '/api/strategies/{id}/interests',
  )
  const deleteMutation = $api.useMutation(
    'delete',
    '/api/strategies/{id}/interests/{ref_kind}/{ref_id}',
  )

  const [formOpen, setFormOpen] = useState(false)
  const [form, setForm] = useState<FormState>(EMPTY_FORM)
  const [formError, setFormError] = useState<string | null>(null)
  const [listError, setListError] = useState<string | null>(null)

  const seeds = interests.filter((i) => i.role === 'seed')
  const derived = interests.filter((i) => i.role === 'derived')

  function invalidate() {
    void queryClient.invalidateQueries({ queryKey: listQueryOptions.queryKey })
  }

  function openForm() {
    setForm(EMPTY_FORM)
    setFormError(null)
    setFormOpen(true)
  }

  function closeForm() {
    setFormOpen(false)
    setFormError(null)
  }

  function handleCreate() {
    const refId = form.refId.trim()
    if (refId === '') {
      setFormError('ref_id は必須です')
      return
    }
    createMutation.mutate(
      {
        params: { path: { id: strategyId } },
        body: {
          ref_kind: form.refKind,
          ref_id: refId,
          role: form.role,
          origin: form.origin,
        },
      },
      {
        onSuccess: () => {
          invalidate()
          closeForm()
        },
        onError: () => {
          setFormError('関心の追加に失敗しました')
        },
      },
    )
  }

  function handleDelete(interest: StrategyInterest) {
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
          invalidate()
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
          onClick={openForm}
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
        {formOpen && (
          <form
            data-testid="interest-form"
            className="space-y-2 border-t border-[color:var(--color-hairline)] pt-3"
            onSubmit={(e) => {
              e.preventDefault()
              handleCreate()
            }}
          >
            <div className="grid grid-cols-2 gap-2">
              <select
                aria-label="ref_kind"
                value={form.refKind}
                onChange={(e) => {
                  setForm({ ...form, refKind: parseRefKind(e.target.value) })
                }}
                className="h-8 w-full rounded-md border border-input bg-transparent px-2 font-mono text-[11px]"
              >
                {REF_KINDS.map((k) => (
                  <option key={k} value={k}>
                    {REF_KIND_JP[k]}
                  </option>
                ))}
              </select>
              <Input
                aria-label="ref_id"
                value={form.refId}
                placeholder="ref_id"
                onChange={(e) => {
                  setForm({ ...form, refId: e.target.value })
                }}
                className="h-8 text-[11px]"
              />
            </div>
            <div className="grid grid-cols-2 gap-2">
              <select
                aria-label="role"
                value={form.role}
                onChange={(e) => {
                  setForm({ ...form, role: parseRole(e.target.value) })
                }}
                className="h-8 w-full rounded-md border border-input bg-transparent px-2 font-mono text-[11px]"
              >
                {ROLES.map((r) => (
                  <option key={r} value={r}>
                    {r}
                  </option>
                ))}
              </select>
              <select
                aria-label="origin"
                value={form.origin}
                onChange={(e) => {
                  setForm({ ...form, origin: parseOrigin(e.target.value) })
                }}
                className="h-8 w-full rounded-md border border-input bg-transparent px-2 font-mono text-[11px]"
              >
                {ORIGINS.map((o) => (
                  <option key={o} value={o}>
                    {ORIGIN_LABEL[o]}
                  </option>
                ))}
              </select>
            </div>
            {formError != null && (
              <p
                data-testid="interest-form-error"
                className="font-mono text-[11px] text-[color:var(--color-accent-strategy)]"
              >
                {formError}
              </p>
            )}
            <div className="flex items-center gap-2">
              <Button type="submit" disabled={createMutation.isPending}>
                {createMutation.isPending ? '追加中…' : '追加'}
              </Button>
              <Button type="button" variant="outline" onClick={closeForm}>
                キャンセル
              </Button>
            </div>
          </form>
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

function InterestRow({
  interest,
  onDelete,
}: {
  interest: StrategyInterest
  onDelete: (interest: StrategyInterest) => void
}) {
  return (
    <li className="flex items-center gap-2">
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
