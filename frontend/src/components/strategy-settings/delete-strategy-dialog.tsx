import { useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { useState } from 'react'

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

interface DeleteStrategyDialogProps {
  strategyId: string
  strategyName: string
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function DeleteStrategyDialog({
  strategyId,
  strategyName,
  open,
  onOpenChange,
}: DeleteStrategyDialogProps) {
  const [confirmText, setConfirmText] = useState('')
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const deleteMutation = $api.useMutation('delete', '/api/strategies/{id}')

  function reset() {
    setConfirmText('')
  }

  function handleDelete() {
    if (confirmText !== strategyName || deleteMutation.isPending) return
    deleteMutation.mutate(
      { params: { path: { id: strategyId } } },
      {
        onSuccess: () => {
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions('get', '/api/strategies').queryKey,
          })
          reset()
          onOpenChange(false)
          void navigate({ to: '/strategies' })
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
        <DialogHeader>
          <DialogTitle>戦略を削除しますか?</DialogTitle>
          <DialogDescription>
            この操作は取り消せません。ノート・アノテーション・トレード・仮説・トリガー・カスタムインジケーター・戦略タスク・関心など、この戦略に紐づくすべてのデータが削除されます。
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-2">
          <label
            htmlFor="delete-confirm-name"
            className="block font-mono text-2xs uppercase tracking-wide text-muted-foreground"
          >
            確認のため戦略名「{strategyName}」を入力してください
          </label>
          <Input
            id="delete-confirm-name"
            autoFocus
            value={confirmText}
            onChange={(e) => {
              setConfirmText(e.target.value)
            }}
          />
        </div>
        {deleteMutation.isError && (
          <p className="text-xs text-primary">削除に失敗しました</p>
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
            type="button"
            variant="destructive"
            disabled={confirmText !== strategyName || deleteMutation.isPending}
            onClick={handleDelete}
          >
            {deleteMutation.isPending ? '削除中…' : '削除する'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
