import { useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import { MarkdownEditor } from '#components/strategy-settings/markdown-editor'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

interface AgentsMdTabProps {
  strategyId: string
}

export function AgentsMdTab({ strategyId }: AgentsMdTabProps) {
  const queryClient = useQueryClient()
  const { data, isPending } = $api.useQuery(
    'get',
    '/api/strategies/{id}/agents-md',
    { params: { path: { id: strategyId } } },
  )
  const mutation = $api.useMutation('put', '/api/strategies/{id}/agents-md')
  const [saveError, setSaveError] = useState<string | null>(null)

  if (isPending) {
    return <Skeleton className="h-80 w-full" />
  }

  return (
    <MarkdownEditor
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
                  '/api/strategies/{id}/agents-md',
                  { params: { path: { id: strategyId } } },
                ).queryKey,
              })
            },
            onError: () => {
              setSaveError('保存に失敗しました')
            },
          },
        )
      }}
    />
  )
}
