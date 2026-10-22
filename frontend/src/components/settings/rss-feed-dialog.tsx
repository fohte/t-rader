import { useQueryClient } from '@tanstack/react-query'
import type { SyntheticEvent } from 'react'
import { useEffect, useState } from 'react'

import { RssFeedDialogView } from '#components/settings/rss-feed-dialog-view'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type RssFeed = components['schemas']['RssFeed']

interface RssFeedDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 編集対象。null なら新規作成モード */
  feed: RssFeed | null
}

function listQueryKey() {
  return $api.queryOptions('get', '/api/rss-feeds').queryKey
}

export function RssFeedDialog({
  open,
  onOpenChange,
  feed,
}: RssFeedDialogProps) {
  const [source, setSource] = useState('')
  const [displayName, setDisplayName] = useState('')
  const [url, setUrl] = useState('')
  const [enabled, setEnabled] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const queryClient = useQueryClient()
  const createMutation = $api.useMutation('post', '/api/rss-feeds')
  const updateMutation = $api.useMutation('patch', '/api/rss-feeds/{id}')

  useEffect(() => {
    if (!open) return
    setError(null)
    if (feed != null) {
      setSource(feed.source)
      setDisplayName(feed.display_name)
      setUrl(feed.url)
      setEnabled(feed.enabled)
    } else {
      setSource('')
      setDisplayName('')
      setUrl('')
      setEnabled(true)
    }
  }, [open, feed])

  const pending = createMutation.isPending || updateMutation.isPending

  function handleSubmit(event: SyntheticEvent<HTMLFormElement>) {
    event.preventDefault()
    setError(null)
    const onError = (err: { error?: string }) => {
      setError(err.error ?? '保存に失敗しました')
    }
    if (feed != null) {
      updateMutation.mutate(
        {
          params: { path: { id: feed.id } },
          body: {
            display_name: displayName.trim(),
            url: url.trim(),
            enabled,
          },
        },
        {
          onSuccess: () => {
            void queryClient.invalidateQueries({ queryKey: listQueryKey() })
            onOpenChange(false)
          },
          onError,
        },
      )
    } else {
      createMutation.mutate(
        {
          body: {
            source: source.trim(),
            display_name: displayName.trim(),
            url: url.trim(),
            enabled,
          },
        },
        {
          onSuccess: () => {
            void queryClient.invalidateQueries({ queryKey: listQueryKey() })
            onOpenChange(false)
          },
          onError,
        },
      )
    }
  }

  return (
    <RssFeedDialogView
      open={open}
      onOpenChange={onOpenChange}
      feed={feed}
      source={source}
      onSourceChange={setSource}
      displayName={displayName}
      onDisplayNameChange={setDisplayName}
      url={url}
      onUrlChange={setUrl}
      enabled={enabled}
      onEnabledChange={setEnabled}
      isPending={pending}
      errorMessage={error}
      onSubmit={handleSubmit}
    />
  )
}
