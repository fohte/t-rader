import { useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import { AgentGraphEditor } from '#components/strategy-settings/agent-graph-editor'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

interface AgentGraphTabProps {
  purpose: string
}

export function AgentGraphTab({ purpose }: AgentGraphTabProps) {
  const queryClient = useQueryClient()
  const { data, isPending } = $api.useQuery(
    'get',
    '/api/agent-configs/{purpose}/agent-graph',
    { params: { path: { purpose } } },
  )
  const mutation = $api.useMutation(
    'put',
    '/api/agent-configs/{purpose}/agent-graph',
  )
  const [saveError, setSaveError] = useState<string | null>(null)

  if (isPending) {
    return <Skeleton className="h-80 w-full" />
  }

  return (
    <AgentGraphEditor
      purpose={purpose}
      initialValue={data?.content ?? ''}
      isSaving={mutation.isPending}
      saveError={saveError}
      onSave={(next) => {
        setSaveError(null)
        mutation.mutate(
          {
            params: { path: { purpose } },
            body: { content: next },
          },
          {
            onSuccess: () => {
              void queryClient.invalidateQueries({
                queryKey: $api.queryOptions(
                  'get',
                  '/api/agent-configs/{purpose}/agent-graph',
                  { params: { path: { purpose } } },
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
