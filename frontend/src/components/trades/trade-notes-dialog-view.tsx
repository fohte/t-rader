import { Link } from '@tanstack/react-router'

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
import type { components } from '#lib/api/schema.gen'

type Note = components['schemas']['Note']
type Trade = components['schemas']['Trade']

export function TradeNotesDialogView({
  open,
  onOpenChange,
  trade,
  linkedNotes,
  candidateNotes,
  search,
  onSearchChange,
  linkedNotesPending,
  candidateNotesPending,
  loadingError,
  operationError,
  operationPending,
  onLink,
  onUnlink,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  trade: Trade | null
  linkedNotes: Note[]
  candidateNotes: Note[]
  search: string
  onSearchChange: (value: string) => void
  linkedNotesPending: boolean
  candidateNotesPending: boolean
  loadingError: boolean
  operationError: string | null
  operationPending: boolean
  onLink: (noteId: string) => void
  onUnlink: (noteId: string) => void
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-screen overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>判断ノートの紐付け</DialogTitle>
          <DialogDescription>
            {trade == null
              ? '取引に関係する判断ノートを管理します。'
              : `${trade.date} / ${trade.symbol} の取引に関係するノートを管理します。`}
          </DialogDescription>
        </DialogHeader>

        {loadingError && (
          <p className="text-sm text-primary">
            ノートの読み込みに失敗しました。
          </p>
        )}
        {operationError != null && (
          <p role="alert" className="text-sm text-primary">
            {operationError}
          </p>
        )}

        <section className="space-y-2">
          <h3 className="font-mono text-xs font-bold uppercase tracking-wider text-muted-foreground-strong">
            紐付け済み ({linkedNotes.length})
          </h3>
          <div className="divide-y divide-border border border-border bg-card">
            {linkedNotesPending ? (
              <p className="px-3 py-4 text-sm text-muted-foreground">
                読み込み中...
              </p>
            ) : linkedNotes.length === 0 ? (
              <p className="px-3 py-4 text-sm text-muted-foreground">
                紐付け済みのノートはありません。
              </p>
            ) : (
              linkedNotes.map((note) => (
                <div
                  key={note.id}
                  className="flex items-center justify-between gap-3 px-3 py-2.5"
                >
                  <Link
                    to="/notes/$noteId"
                    params={{ noteId: note.id }}
                    className="min-w-0 flex-1 truncate text-sm text-foreground hover:text-primary hover:underline"
                  >
                    {note.title}
                  </Link>
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    disabled={operationPending}
                    onClick={() => {
                      onUnlink(note.id)
                    }}
                  >
                    解除
                  </Button>
                </div>
              ))
            )}
          </div>
        </section>

        <section className="space-y-2">
          <h3 className="font-mono text-xs font-bold uppercase tracking-wider text-muted-foreground-strong">
            同じ戦略のノート
          </h3>
          <Input
            aria-label="判断ノートを検索"
            placeholder="タイトルで絞り込み"
            value={search}
            onChange={(event) => {
              onSearchChange(event.currentTarget.value)
            }}
          />
          <div className="divide-y divide-border border border-border bg-card">
            {candidateNotesPending ? (
              <p className="px-3 py-4 text-sm text-muted-foreground">
                読み込み中...
              </p>
            ) : candidateNotes.length === 0 ? (
              <p className="px-3 py-4 text-sm text-muted-foreground">
                {search.trim() === ''
                  ? '紐付け可能なノートはありません。'
                  : '検索条件に一致するノートはありません。'}
              </p>
            ) : (
              candidateNotes.map((note) => (
                <div
                  key={note.id}
                  className="flex items-center justify-between gap-3 px-3 py-2.5"
                >
                  <span className="min-w-0 flex-1 truncate text-sm text-foreground">
                    {note.title}
                  </span>
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    disabled={
                      operationPending || linkedNotesPending || loadingError
                    }
                    onClick={() => {
                      onLink(note.id)
                    }}
                  >
                    紐付け
                  </Button>
                </div>
              ))
            )}
          </div>
        </section>

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            disabled={operationPending}
            onClick={() => {
              onOpenChange(false)
            }}
          >
            閉じる
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
