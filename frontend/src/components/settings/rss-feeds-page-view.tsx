import { Button } from '@fohte/ui/button'
import { Checkbox } from '@fohte/ui/checkbox'
import { Pencil, Plus, Trash2 } from 'lucide-react'

import { RssFeedDialog } from '#components/settings/rss-feed-dialog'
import { SettingsBackLink } from '#components/settings/settings-back-link'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'

type RssFeed = components['schemas']['RssFeed']

export interface RssFeedsPageViewProps {
  feeds: RssFeed[] | undefined
  isPending: boolean
  dialogOpen: boolean
  editingFeed: RssFeed | null
  onAddFeed: () => void
  onToggleEnabled: (feed: RssFeed) => void
  onEditFeed: (feed: RssFeed) => void
  onDeleteFeed: (feed: RssFeed) => void
  onDialogOpenChange: (open: boolean) => void
}

export function RssFeedsPageView({
  feeds,
  isPending,
  dialogOpen,
  editingFeed,
  onAddFeed,
  onToggleEnabled,
  onEditFeed,
  onDeleteFeed,
  onDialogOpenChange,
}: RssFeedsPageViewProps) {
  return (
    <div className="space-y-5">
      <div>
        <SettingsBackLink />
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
        <Button onClick={onAddFeed} size="sm">
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
                      <Checkbox
                        checked={feed.enabled}
                        onCheckedChange={() => {
                          onToggleEnabled(feed)
                        }}
                      />
                      <span className="sr-only">
                        {feed.display_name} の有効状態
                      </span>
                      <span>{feed.enabled ? 'on' : 'off'}</span>
                    </label>
                  </td>
                  <td className="px-3 py-2 text-right">
                    <div className="flex justify-end gap-1">
                      <Button
                        variant="plain"
                        className="size-7"
                        aria-label="編集"
                        onClick={() => {
                          onEditFeed(feed)
                        }}
                      >
                        <Pencil className="size-3.5" />
                      </Button>
                      <Button
                        variant="plain"
                        className="size-7"
                        aria-label="削除"
                        onClick={() => {
                          onDeleteFeed(feed)
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
        onOpenChange={onDialogOpenChange}
        feed={editingFeed}
      />
    </div>
  )
}
