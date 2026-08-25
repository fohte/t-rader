import { useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import { AgentGraphEditor } from '#components/strategy-settings/agent-graph-editor'
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

  if (isPending) {
    return <Skeleton className="h-80 w-full" />
  }

  return (
    <AgentGraphEditor
      strategyId={strategyId}
      initialValue={data?.content ?? ''}
      isSaving={mutation.isPending}
      saveError={saveError}
      onSave={(next) => {
        setSaveError(null)
        mutation.mutate(
          {
            params: { path: { id: strategyId } },
            body: { content: next },
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
            onError: (err: unknown) => {
              // openapi-react-query は失敗レスポンスの JSON (ErrorResponse) を想定するが、
              // fetch 自体の例外 (ネットワーク断など) では素の Error が渡ることがある
              setSaveError(
                typeof err === 'object' &&
                  err != null &&
                  'error' in err &&
                  typeof err.error === 'string'
                  ? err.error
                  : '保存に失敗しました',
              )
            },
          },
        )
      }}
    />
  )
}
