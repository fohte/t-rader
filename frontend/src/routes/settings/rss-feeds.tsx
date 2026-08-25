import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute, Link } from '@tanstack/react-router'
import { Pencil, Plus, Trash2 } from 'lucide-react'
import { useState } from 'react'

import { RssFeedDialog } from '#components/settings/rss-feed-dialog'
import { Button } from '#components/ui/button'
import { Skeleton } from '#components/ui/skeleton'
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
    <div className="space-y-5">
      <div>
        <Link
          to="/strategies"
          className="font-mono text-xs text-muted-foreground hover:text-foreground"
        >
          &lt; 戦略一覧に戻る
        </Link>
      </div>
      <header className="flex items-end justify-between gap-3">
        <div>
          <h1 className="mb-1 text-2xl font-bold leading-tight tracking-tight">
            設定 — RSS フィード
          </h1>
          <p className="text-sm text-muted-foreground-strong">
            ニュース集約 (1 時間ごと) が読みに行く公開 RSS の一覧を管理します。
          </p>
        </div>
        <Button onClick={openCreate} size="sm">
          <Plus className="size-3.5" /> フィードを追加
        </Button>
      </header>

      {isPending ? (
        <Skeleton className="h-50 w-full" />
      ) : (feeds ?? []).length === 0 ? (
        <div className="border border-dashed border-border p-6 text-center text-sm text-muted-foreground">
          まだフィードが登録されていません。「フィードを追加」から登録してください。
        </div>
      ) : (
        <div className="overflow-x-auto border border-border">
          <table className="w-full font-mono text-xs">
            <thead className="bg-surface-strong text-muted-foreground">
              <tr>
                <th className="px-3 py-2 text-left">表示名</th>
                <th className="px-3 py-2 text-left">source</th>
                <th className="px-3 py-2 text-left">URL</th>
                <th className="px-3 py-2 text-left">有効</th>
                <th className="px-3 py-2 text-right">操作</th>
              </tr>
            </thead>
            <tbody>
              {(feeds ?? []).map((feed) => (
                <tr key={feed.id} className="border-t border-border">
                  <td className="px-3 py-2 text-foreground">
                    {feed.display_name}
                  </td>
                  <td className="px-3 py-2 text-muted-foreground-strong">
                    {feed.source}
                  </td>
                  <td className="max-w-80 truncate px-3 py-2 text-muted-foreground-strong">
                    <a
                      href={feed.url}
                      target="_blank"
                      rel="noreferrer"
                      className="hover:underline"
                    >
                      {feed.url}
                    </a>
                  </td>
                  <td className="px-3 py-2">
                    <label className="inline-flex items-center gap-1.5">
                      <input
                        type="checkbox"
                        checked={feed.enabled}
                        onChange={() => {
                          toggleEnabled(feed)
                        }}
                      />
                      <span>{feed.enabled ? 'on' : 'off'}</span>
                    </label>
                  </td>
                  <td className="px-3 py-2 text-right">
                    <div className="flex justify-end gap-1">
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label="編集"
                        onClick={() => {
                          openEdit(feed)
                        }}
                      >
                        <Pencil className="size-3.5" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label="削除"
                        onClick={() => {
                          handleDelete(feed)
                        }}
                      >
                        <Trash2 className="size-3.5" />
                      </Button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <RssFeedDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        feed={editing}
      />
    </div>
  )
}
