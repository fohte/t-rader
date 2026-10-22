import { Button } from '@fohte/ui/button'
import { Checkbox } from '@fohte/ui/checkbox'
import { Input } from '@fohte/ui/input'
import type { SyntheticEvent } from 'react'

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '#components/ui/dialog'
import type { components } from '#lib/api/schema.gen'

type RssFeed = components['schemas']['RssFeed']

export interface RssFeedDialogViewProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  feed: RssFeed | null
  source: string
  onSourceChange: (value: string) => void
  displayName: string
  onDisplayNameChange: (value: string) => void
  url: string
  onUrlChange: (value: string) => void
  enabled: boolean
  onEnabledChange: (enabled: boolean) => void
  isPending: boolean
  errorMessage: string | null
  onSubmit: (event: SyntheticEvent<HTMLFormElement>) => void
}

export function RssFeedDialogView({
  open,
  onOpenChange,
  feed,
  source,
  onSourceChange,
  displayName,
  onDisplayNameChange,
  url,
  onUrlChange,
  enabled,
  onEnabledChange,
  isPending,
  errorMessage,
  onSubmit,
}: RssFeedDialogViewProps) {
  const isEdit = feed != null

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <form onSubmit={onSubmit} className="space-y-4">
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
              onChange={(event) => {
                onSourceChange(event.target.value)
              }}
              placeholder="例: sample-feed"
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
              onChange={(event) => {
                onDisplayNameChange(event.target.value)
              }}
              placeholder="例: Example Feed"
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
              onChange={(event) => {
                onUrlChange(event.target.value)
              }}
              placeholder="https://feeds.example.invalid/rss"
            />
          </div>
          <div className="flex items-center gap-2">
            <Checkbox
              id="rss-enabled"
              aria-label="有効 (集約対象)"
              checked={enabled}
              onCheckedChange={onEnabledChange}
            />
            <label
              htmlFor="rss-enabled"
              className="font-mono text-xs text-muted-foreground-strong"
            >
              有効 (集約対象)
            </label>
          </div>
          {errorMessage != null && (
            <p role="alert" className="text-xs text-destructive">
              {errorMessage}
            </p>
          )}
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
            <Button type="submit" disabled={isPending}>
              {isPending ? '保存中…' : isEdit ? '更新' : '作成'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
