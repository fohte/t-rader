import { useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { $api } from '@/lib/api/client'

interface CreateHypothesisDialogProps {
  strategyId: string
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function CreateHypothesisDialog({
  strategyId,
  open,
  onOpenChange,
}: CreateHypothesisDialogProps) {
  const [title, setTitle] = useState('')
  const [body, setBody] = useState('')
  const [formError, setFormError] = useState<string | null>(null)
  const queryClient = useQueryClient()
  const createMutation = $api.useMutation(
    'post',
    '/api/strategies/{id}/hypotheses',
  )

  function reset() {
    setTitle('')
    setBody('')
    setFormError(null)
  }

  function handleSubmit(e: React.SyntheticEvent) {
    e.preventDefault()
    if (createMutation.isPending) return
    const trimmedTitle = title.trim()
    const trimmedBody = body.trim()
    if (trimmedTitle === '') {
      setFormError('title は必須です')
      return
    }
    if (trimmedBody === '') {
      setFormError('body は必須です')
      return
    }
    createMutation.mutate(
      {
        params: { path: { id: strategyId } },
        body: { title: trimmedTitle, body: trimmedBody },
      },
      {
        onSuccess: () => {
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions(
              'get',
              '/api/strategies/{id}/hypotheses',
              { params: { path: { id: strategyId } } },
            ).queryKey,
          })
          reset()
          onOpenChange(false)
        },
        onError: () => {
          setFormError('仮説の作成に失敗しました')
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
            <DialogTitle>新しい仮説を作る</DialogTitle>
            <DialogDescription>
              関心の組合せに対する検証可能な主張を書きます。
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <label
              htmlFor="hypothesis-title"
              className="block font-mono text-[11px] uppercase tracking-wide text-[color:var(--color-text-tertiary)]"
            >
              title *
            </label>
            <Input
              id="hypothesis-title"
              autoFocus
              value={title}
              onChange={(e) => {
                setTitle(e.target.value)
              }}
              placeholder="例: USD/JPY 押し目買い"
            />
          </div>
          <div className="space-y-2">
            <label
              htmlFor="hypothesis-body"
              className="block font-mono text-[11px] uppercase tracking-wide text-[color:var(--color-text-tertiary)]"
            >
              body (Markdown) *
            </label>
            <textarea
              id="hypothesis-body"
              rows={5}
              value={body}
              onChange={(e) => {
                setBody(e.target.value)
              }}
              placeholder="主張の根拠と検証方法"
              className="w-full border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-secondary)] px-3 py-2 font-mono text-[12.5px] text-[color:var(--color-text-primary)] outline-none focus:border-[color:var(--color-text-tertiary)]"
            />
          </div>
          {formError != null && (
            <p
              data-testid="create-hypothesis-error"
              className="text-[12px] text-[color:var(--color-accent-strategy)]"
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
              {createMutation.isPending ? '作成中…' : '作成'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
