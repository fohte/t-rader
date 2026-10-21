import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'

import { RssFeedsPageView } from '#components/settings/rss-feeds-page-view'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type RssFeed = components['schemas']['RssFeed']

export const Route = createFileRoute('/settings/rss-feeds')({
  component: RssFeedsSettingsPage,
})

function RssFeedsSettingsPage() {
  const queryClient = useQueryClient()
  const { data: feeds, isPending } = $api.useQuery('get', '/api/rss-feeds')
  const updateMutation = $api.useMutation('patch', '/api/rss-feeds/{id}')
  const deleteMutation = $api.useMutation('delete', '/api/rss-feeds/{id}')
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editing, setEditing] = useState<RssFeed | null>(null)

  function invalidate() {
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/rss-feeds').queryKey,
    })
  }

  function toggleEnabled(feed: RssFeed) {
    updateMutation.mutate(
      {
        params: { path: { id: feed.id } },
        body: { enabled: !feed.enabled },
      },
      {
        onSuccess: invalidate,
        onError: (err) => {
          invalidate()
          window.alert(
            `${feed.display_name} の更新に失敗しました: ${err.error || '不明なエラー'}`,
          )
        },
      },
    )
  }

  function handleDelete(feed: RssFeed) {
    if (!window.confirm(`${feed.display_name} を削除します。よろしいですか?`))
      return
    deleteMutation.mutate(
      { params: { path: { id: feed.id } } },
      {
        onSuccess: invalidate,
        onError: (err) => {
          invalidate()
          window.alert(
            `${feed.display_name} の削除に失敗しました: ${err.error || '不明なエラー'}`,
          )
        },
      },
    )
  }

  function openCreate() {
    setEditing(null)
    setDialogOpen(true)
  }

  function openEdit(feed: RssFeed) {
    setEditing(feed)
    setDialogOpen(true)
  }

  return (
    <RssFeedsPageView
      feeds={feeds}
      isPending={isPending}
      dialogOpen={dialogOpen}
      editingFeed={editing}
      onAddFeed={openCreate}
      onToggleEnabled={toggleEnabled}
      onEditFeed={openEdit}
      onDeleteFeed={handleDelete}
      onDialogOpenChange={setDialogOpen}
    />
  )
}
