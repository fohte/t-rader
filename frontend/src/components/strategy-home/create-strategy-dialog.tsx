import { useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
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

interface CreateStrategyDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function CreateStrategyDialog({
  open,
  onOpenChange,
}: CreateStrategyDialogProps) {
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const createMutation = $api.useMutation('post', '/api/strategies')

  function reset() {
    setName('')
    setDescription('')
  }

  function handleSubmit(e: React.SyntheticEvent) {
    e.preventDefault()
    const trimmed = name.trim()
    if (trimmed === '' || createMutation.isPending) return
    createMutation.mutate(
      {
        body: {
          name: trimmed,
          description: description.trim() === '' ? null : description.trim(),
        },
      },
      {
        onSuccess: (created) => {
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions('get', '/api/strategies').queryKey,
          })
          reset()
          onOpenChange(false)
          void navigate({
            to: '/strategies/$id',
            params: { id: created.id },
          })
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
            <DialogTitle>新しい戦略を作る</DialogTitle>
            <DialogDescription>
              名前と説明を入れて作成します。シード関心は後から設定で追加できます。
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <label
              htmlFor="strategy-name"
              className="block font-mono text-[11px] uppercase tracking-wide text-[color:var(--color-text-tertiary)]"
            >
              戦略名 *
            </label>
            <Input
              id="strategy-name"
              required
              autoFocus
              value={name}
              onChange={(e) => {
                setName(e.target.value)
              }}
              placeholder="例: 半導体短期スイング"
            />
          </div>
          <div className="space-y-2">
            <label
              htmlFor="strategy-description"
              className="block font-mono text-[11px] uppercase tracking-wide text-[color:var(--color-text-tertiary)]"
            >
              説明
            </label>
            <textarea
              id="strategy-description"
              rows={3}
              value={description}
              onChange={(e) => {
                setDescription(e.target.value)
              }}
              placeholder="この戦略の狙い、対象、期間など"
              className="w-full border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-secondary)] px-3 py-2 text-[13px] text-[color:var(--color-text-primary)] outline-none focus:border-[color:var(--color-text-tertiary)]"
            />
          </div>
          {createMutation.isError && (
            <p className="text-[12px] text-[color:var(--color-accent-strategy)]">
              作成に失敗しました
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
            <Button
              type="submit"
              disabled={name.trim() === '' || createMutation.isPending}
            >
              {createMutation.isPending ? '作成中…' : '作成'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
