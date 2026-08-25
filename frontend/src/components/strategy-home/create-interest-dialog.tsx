import { useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import { ORIGIN_LABEL } from '#components/strategy-home/interest-tree'
import { Button } from '#components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '#components/ui/dialog'
import { Input } from '#components/ui/input'
import { $api } from '#lib/api/client'
import { REF_KIND_JP, type RefKind } from '#lib/strategy-mock'

const REF_KINDS: RefKind[] = ['stock', 'indicator', 'sector', 'theme']
const ROLES = ['seed', 'derived'] as const
const ORIGINS = ['human', 'llm'] as const

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

interface CreateInterestDialogProps {
  strategyId: string
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function CreateInterestDialog({
  strategyId,
  open,
  onOpenChange,
}: CreateInterestDialogProps) {
  const queryClient = useQueryClient()
  const [form, setForm] = useState<FormState>(EMPTY_FORM)
  const [formError, setFormError] = useState<string | null>(null)
  const createMutation = $api.useMutation(
    'post',
    '/api/strategies/{id}/interests',
  )

  function reset() {
    setForm(EMPTY_FORM)
    setFormError(null)
  }

  function handleSubmit(e: React.SyntheticEvent) {
    e.preventDefault()
    if (createMutation.isPending) return
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
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions(
              'get',
              '/api/strategies/{id}/interests',
              { params: { path: { id: strategyId } } },
            ).queryKey,
          })
          reset()
          onOpenChange(false)
        },
        onError: () => {
          setFormError('関心の追加に失敗しました')
        },
      },
    )
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        if (!v) reset()
        onOpenChange(v)
      }}
    >
      <DialogContent>
        <form onSubmit={handleSubmit} className="space-y-4">
          <DialogHeader>
            <DialogTitle>関心を追加する</DialogTitle>
            <DialogDescription>
              監視対象にしたい銘柄・指標・セクター・テーマを登録します。
            </DialogDescription>
          </DialogHeader>
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-2">
              <label
                htmlFor="interest-ref-kind"
                className="block font-mono text-2xs uppercase tracking-wide text-muted-foreground"
              >
                ref_kind
              </label>
              <select
                id="interest-ref-kind"
                value={form.refKind}
                onChange={(e) => {
                  setForm({ ...form, refKind: parseRefKind(e.target.value) })
                }}
                className="h-9 w-full rounded-md border border-input bg-transparent px-3 font-mono text-xs"
              >
                {REF_KINDS.map((k) => (
                  <option key={k} value={k}>
                    {REF_KIND_JP[k]}
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-2">
              <label
                htmlFor="interest-ref-id"
                className="block font-mono text-2xs uppercase tracking-wide text-muted-foreground"
              >
                ref_id
              </label>
              <Input
                id="interest-ref-id"
                autoFocus
                value={form.refId}
                placeholder="例: 7203"
                onChange={(e) => {
                  setForm({ ...form, refId: e.target.value })
                }}
              />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-2">
              <label
                htmlFor="interest-role"
                className="block font-mono text-2xs uppercase tracking-wide text-muted-foreground"
              >
                role
              </label>
              <select
                id="interest-role"
                value={form.role}
                onChange={(e) => {
                  setForm({ ...form, role: parseRole(e.target.value) })
                }}
                className="h-9 w-full rounded-md border border-input bg-transparent px-3 font-mono text-xs"
              >
                {ROLES.map((r) => (
                  <option key={r} value={r}>
                    {r}
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-2">
              <label
                htmlFor="interest-origin"
                className="block font-mono text-2xs uppercase tracking-wide text-muted-foreground"
              >
                origin
              </label>
              <select
                id="interest-origin"
                value={form.origin}
                onChange={(e) => {
                  setForm({ ...form, origin: parseOrigin(e.target.value) })
                }}
                className="h-9 w-full rounded-md border border-input bg-transparent px-3 font-mono text-xs"
              >
                {ORIGINS.map((o) => (
                  <option key={o} value={o}>
                    {ORIGIN_LABEL[o]}
                  </option>
                ))}
              </select>
            </div>
          </div>
          {formError != null && (
            <p
              data-testid="create-interest-error"
              className="text-xs text-primary"
            >
              {formError}
            </p>
          )}
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => {
                onOpenChange(false)
              }}
            >
              キャンセル
            </Button>
            <Button type="submit" disabled={createMutation.isPending}>
              {createMutation.isPending ? '追加中…' : '追加'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
