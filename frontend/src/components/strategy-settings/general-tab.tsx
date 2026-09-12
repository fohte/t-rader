import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'

import { DeleteStrategyDialog } from '#components/strategy-settings/delete-strategy-dialog'
import { Button } from '#components/ui/button'
import { Input } from '#components/ui/input'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

interface GeneralTabProps {
  strategyId: string
}

export function GeneralTab({ strategyId }: GeneralTabProps) {
  const queryClient = useQueryClient()
  const { data: strategy, isPending } = $api.useQuery(
    'get',
    '/api/strategies/{id}',
    { params: { path: { id: strategyId } } },
  )
  const updateMutation = $api.useMutation('patch', '/api/strategies/{id}')

  const initialName = strategy?.name ?? ''
  const initialDescription = strategy?.description ?? ''
  const [name, setName] = useState(initialName)
  const [description, setDescription] = useState(initialDescription)
  // 前回 GET から作った初期値。これと現在値が一致していれば「ユーザー未編集」と判定できる
  const lastInitialRef = useRef({
    name: initialName,
    description: initialDescription,
  })
  const [saveError, setSaveError] = useState<string | null>(null)
  const [deleteOpen, setDeleteOpen] = useState(false)

  useEffect(() => {
    if (
      name === lastInitialRef.current.name &&
      description === lastInitialRef.current.description
    ) {
      setName(initialName)
      setDescription(initialDescription)
    }
    lastInitialRef.current = {
      name: initialName,
      description: initialDescription,
    }
  }, [initialName, initialDescription, name, description])

  if (isPending) {
    return <Skeleton className="h-60 w-full max-w-lg" />
  }

  if (strategy == null) {
    return (
      <div className="font-mono text-sm text-muted-foreground">
        戦略が見つかりませんでした。
      </div>
    )
  }

  const trimmedName = name.trim()
  const trimmedDescription = description.trim()
  const hasChanges =
    trimmedName !== strategy.name ||
    trimmedDescription !== (strategy.description ?? '')

  function handleSave() {
    if (strategy == null) return
    if (trimmedName === '' || !hasChanges || updateMutation.isPending) return

    const body: { name?: string; description?: string } = {}
    if (trimmedName !== strategy.name) body.name = trimmedName
    if (trimmedDescription !== (strategy.description ?? '')) {
      body.description = trimmedDescription
    }

    setSaveError(null)
    updateMutation.mutate(
      { params: { path: { id: strategyId } }, body },
      {
        onSuccess: () => {
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions('get', '/api/strategies/{id}', {
              params: { path: { id: strategyId } },
            }).queryKey,
          })
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions('get', '/api/strategies').queryKey,
          })
        },
        onError: () => {
          setSaveError('保存に失敗しました')
        },
      },
    )
  }

  return (
    <div className="max-w-lg space-y-8">
      <div className="space-y-4">
        <div className="space-y-2">
          <label
            htmlFor="strategy-name"
            className="block font-mono text-2xs uppercase tracking-wide text-muted-foreground"
          >
            戦略名 *
          </label>
          <Input
            id="strategy-name"
            required
            value={name}
            onChange={(e) => {
              setName(e.target.value)
            }}
          />
        </div>
        <div className="space-y-2">
          <label
            htmlFor="strategy-description"
            className="block font-mono text-2xs uppercase tracking-wide text-muted-foreground"
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
            className="w-full border border-border bg-bg-secondary px-3 py-2 text-sm text-foreground outline-none focus:border-muted-foreground"
          />
        </div>
        <div className="flex items-center gap-3">
          <Button
            type="button"
            onClick={handleSave}
            disabled={
              trimmedName === '' || !hasChanges || updateMutation.isPending
            }
          >
            {updateMutation.isPending ? '保存中…' : '保存'}
          </Button>
          {saveError != null && (
            <span data-testid="save-error" className="text-xs text-primary">
              {saveError}
            </span>
          )}
        </div>
      </div>

      <div className="space-y-3 border-t border-destructive/30 pt-6">
        <h2 className="font-mono text-2xs uppercase tracking-wider text-destructive">
          危険な操作
        </h2>
        <p className="text-sm text-muted-foreground-strong">
          戦略を削除すると、紐づくノート・アノテーション・トレード・仮説・トリガー・カスタムインジケーター・戦略タスク・関心もすべて削除されます。この操作は取り消せません。
        </p>
        <Button
          type="button"
          variant="destructive"
          onClick={() => {
            setDeleteOpen(true)
          }}
        >
          戦略を削除
        </Button>
      </div>

      <DeleteStrategyDialog
        strategyId={strategyId}
        strategyName={strategy.name}
        open={deleteOpen}
        onOpenChange={setDeleteOpen}
      />
    </div>
  )
}
