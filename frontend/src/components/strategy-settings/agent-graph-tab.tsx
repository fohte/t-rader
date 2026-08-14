import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'

import { CodeEditor } from '#components/indicators/code-editor'
import { Button } from '#components/ui/button'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

interface AgentGraphTabProps {
  strategyId: string
}

export function AgentGraphTab({ strategyId }: AgentGraphTabProps) {
  const queryClient = useQueryClient()
  const { data, isPending } = $api.useQuery(
    'get',
    '/api/strategies/{id}/agent-graph',
    { params: { path: { id: strategyId } } },
  )
  const mutation = $api.useMutation('put', '/api/strategies/{id}/agent-graph')
  const [saveError, setSaveError] = useState<string | null>(null)

  const initialValue = data?.content ?? ''
  const [value, setValue] = useState(initialValue)
  // 前回親から受け取った initialValue。これと value が一致していれば「ユーザー未編集」と判定できる
  const lastInitialValueRef = useRef(initialValue)
  const dirty = value !== initialValue

  useEffect(() => {
    if (value === lastInitialValueRef.current) {
      setValue(initialValue)
    }
    lastInitialValueRef.current = initialValue
  }, [initialValue, value])

  useEffect(() => {
    if (!dirty) return
    function handler(e: BeforeUnloadEvent) {
      e.preventDefault()
    }
    window.addEventListener('beforeunload', handler)
    return () => {
      window.removeEventListener('beforeunload', handler)
    }
  }, [dirty])

  if (isPending) {
    return <Skeleton className="h-[320px] w-full" />
  }

  function handleSave() {
    if (!dirty || mutation.isPending) return
    setSaveError(null)
    mutation.mutate(
      {
        params: { path: { id: strategyId } },
        body: { content: value },
      },
      {
        onSuccess: () => {
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions(
              'get',
              '/api/strategies/{id}/agent-graph',
              { params: { path: { id: strategyId } } },
            ).queryKey,
          })
        },
        onError: (err) => {
          setSaveError(err.error)
        },
      },
    )
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <label className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
          agent_graph (YAML)
        </label>
        {dirty && (
          <span
            data-testid="dirty-indicator"
            className="font-mono text-[11px] text-[color:var(--color-accent-strategy)]"
          >
            未保存の変更あり
          </span>
        )}
      </div>
      <CodeEditor
        language="yaml"
        value={value}
        onChange={setValue}
        testId="agent-graph-editor"
        ariaLabel="agent_graph"
        height={480}
      />
      <div className="flex items-center gap-3">
        <Button
          type="button"
          onClick={handleSave}
          disabled={!dirty || mutation.isPending}
        >
          {mutation.isPending ? '保存中…' : '保存'}
        </Button>
        {saveError != null && (
          <span
            data-testid="save-error"
            className="whitespace-pre-wrap font-mono text-[12px] text-[color:var(--color-accent-strategy)]"
          >
            {saveError}
          </span>
        )}
      </div>
    </div>
  )
}
