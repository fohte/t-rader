import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
import { useState } from 'react'

import { Button } from '#components/ui/button'
import { Input } from '#components/ui/input'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/settings/agent-configs/')({
  component: AgentConfigsSettingsPage,
})

function AgentConfigsSettingsPage() {
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const { data: agentConfigs, isPending } = $api.useQuery(
    'get',
    '/api/agent-configs',
  )
  const createMutation = $api.useMutation('post', '/api/agent-configs')
  const deleteMutation = $api.useMutation(
    'delete',
    '/api/agent-configs/{purpose}',
  )

  const [newPurpose, setNewPurpose] = useState('')
  const [createError, setCreateError] = useState<string | null>(null)

  function invalidate() {
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/agent-configs').queryKey,
    })
  }

  function handleCreate() {
    const trimmed = newPurpose.trim()
    if (trimmed === '' || createMutation.isPending) return
    setCreateError(null)
    createMutation.mutate(
      { body: { purpose: trimmed } },
      {
        onSuccess: (created) => {
          invalidate()
          setNewPurpose('')
          void navigate({
            to: '/settings/agent-configs/$purpose',
            params: { purpose: created.purpose },
          })
        },
        onError: (err) => {
          setCreateError(err.error || '作成に失敗しました')
        },
      },
    )
  }

  function handleDelete(purpose: string) {
    if (!window.confirm(`目的 "${purpose}" の agent 設定を削除しますか?`))
      return
    deleteMutation.mutate(
      { params: { path: { purpose } } },
      {
        onSuccess: invalidate,
        onError: () => {
          window.alert(`"${purpose}" の削除に失敗しました`)
        },
      },
    )
  }

  return (
    <div className="space-y-5">
      <div>
        <Link
          to="/settings"
          className="font-mono text-xs text-muted-foreground hover:text-foreground"
        >
          &lt; 設定に戻る
        </Link>
      </div>
      <header>
        <h1 className="mb-1 text-2xl font-bold leading-tight tracking-tight">
          設定 — Agent 設定
        </h1>
        <p className="text-sm text-muted-foreground-strong">
          目的ごとに AGENTS.md / skills / agent graph
          を管理します。目的は資金の区分とは独立しています。
        </p>
      </header>

      <div className="max-w-sm space-y-1.5">
        <label
          htmlFor="new-agent-config-purpose"
          className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
        >
          新しい目的
        </label>
        <div className="flex gap-2">
          <Input
            id="new-agent-config-purpose"
            value={newPurpose}
            placeholder="例: explore"
            aria-invalid={createError != null}
            onChange={(e) => {
              setNewPurpose(e.target.value)
              if (createError != null) setCreateError(null)
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault()
                handleCreate()
              }
            }}
          />
          <Button
            type="button"
            onClick={handleCreate}
            disabled={createMutation.isPending}
          >
            追加
          </Button>
        </div>
        {createError != null && (
          <p
            data-testid="create-error"
            className="font-mono text-2xs text-primary"
          >
            {createError}
          </p>
        )}
      </div>

      {isPending ? (
        <Skeleton className="h-40 w-full" />
      ) : (agentConfigs ?? []).length === 0 ? (
        <div className="border border-dashed border-border p-6 text-center text-sm text-muted-foreground">
          まだ Agent
          設定が登録されていません。上のフォームから追加してください。
        </div>
      ) : (
        <ul data-testid="agent-config-list" className="border border-border">
          {(agentConfigs ?? []).map((c) => (
            <li
              key={c.purpose}
              className="flex items-center justify-between border-b border-border px-1 last:border-b-0"
            >
              <Link
                to="/settings/agent-configs/$purpose"
                params={{ purpose: c.purpose }}
                className="flex-1 truncate px-3 py-2 font-mono text-sm hover:text-primary"
              >
                {c.purpose}
              </Link>
              <button
                type="button"
                onClick={() => {
                  handleDelete(c.purpose)
                }}
                aria-label={`"${c.purpose}" を削除`}
                className="px-3 py-1 font-mono text-2xs text-muted-foreground hover:text-primary"
              >
                削除
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
