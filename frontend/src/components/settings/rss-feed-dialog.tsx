import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'

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
  const isEdit = feed != null
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

  function handleSubmit(e: React.SyntheticEvent) {
    e.preventDefault()
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
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <form onSubmit={handleSubmit} className="space-y-4">
          <DialogHeader>
            <DialogTitle>
              {isEdit ? 'RSS フィードを編集' : 'RSS フィードを追加'}
            </DialogTitle>
            <DialogDescription>
              ニュース集約 (1 時間ごと) の対象フィードです。enabled=false
              にすると次回 tick から外れます。
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <label
              htmlFor="rss-source"
              className="block font-mono text-2xs uppercase tracking-wide text-muted-foreground"
            >
              source (slug, 不変) *
            </label>
            <Input
              id="rss-source"
              required
              autoFocus={!isEdit}
              disabled={isEdit}
              value={source}
              onChange={(e) => {
                setSource(e.target.value)
              }}
              placeholder="例: bloomberg-jp"
              pattern="[a-z0-9_-]+"
              title="小文字英数字とハイフン/アンダースコアのみ"
            />
          </div>
          <div className="space-y-2">
            <label
              htmlFor="rss-display-name"
              className="block font-mono text-2xs uppercase tracking-wide text-muted-foreground"
            >
              表示名 *
            </label>
            <Input
              id="rss-display-name"
              required
              value={displayName}
              onChange={(e) => {
                setDisplayName(e.target.value)
              }}
              placeholder="例: Bloomberg JP"
            />
          </div>
          <div className="space-y-2">
            <label
              htmlFor="rss-url"
              className="block font-mono text-2xs uppercase tracking-wide text-muted-foreground"
            >
              URL *
            </label>
            <Input
              id="rss-url"
              required
              type="url"
              value={url}
              onChange={(e) => {
                setUrl(e.target.value)
              }}
              placeholder="https://feeds.example.com/rss"
            />
          </div>
          <div className="flex items-center gap-2">
            <input
              id="rss-enabled"
              type="checkbox"
              checked={enabled}
              onChange={(e) => {
                setEnabled(e.target.checked)
              }}
            />
            <label
              htmlFor="rss-enabled"
              className="font-mono text-xs text-muted-foreground-strong"
            >
              有効 (集約対象)
            </label>
          </div>
          {error != null && <p className="text-xs text-primary">{error}</p>}
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
            <Button type="submit" disabled={pending}>
              {pending ? '保存中…' : isEdit ? '更新' : '作成'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
