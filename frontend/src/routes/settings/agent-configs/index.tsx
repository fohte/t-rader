import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useState } from 'react'

import { SettingsBackLink } from '#components/settings/settings-back-link'
import { AgentConfigsIndexView } from '#components/strategy-settings/agent-configs-index-view'
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

  function handlePurposeChange(value: string) {
    setNewPurpose(value)
    if (createError != null) setCreateError(null)
  }

  return (
    <div className="space-y-5">
      <div>
        <SettingsBackLink />
      </div>
      <AgentConfigsIndexView
        purposes={(agentConfigs ?? []).map((config) => config.purpose)}
        isPending={isPending}
        newPurpose={newPurpose}
        isCreating={createMutation.isPending}
        createError={createError}
        onPurposeChange={handlePurposeChange}
        onCreate={handleCreate}
        onDelete={handleDelete}
      />
    </div>
  )
}
