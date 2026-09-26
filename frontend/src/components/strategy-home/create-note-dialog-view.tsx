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

type NoteKind = components['schemas']['NoteKind']

export interface CreateNoteDialogViewProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  body: string
  kind: string
  noteKinds: NoteKind[]
  noteKindsPending: boolean
  noteKindsError: boolean
  formError: string | null
  isSubmitting: boolean
  onTitleChange: (value: string) => void
  onBodyChange: (value: string) => void
  onKindChange: (value: string) => void
  onSubmit: (event: React.SyntheticEvent) => void
}

export function CreateNoteDialogView({
  open,
  onOpenChange,
  title,
  body,
  kind,
  noteKinds,
  noteKindsPending,
  noteKindsError,
  formError,
  isSubmitting,
  onTitleChange,
  onBodyChange,
  onKindChange,
  onSubmit,
}: CreateNoteDialogViewProps) {
  return (
    <>
      <Button
        onClick={() => {
          onOpenChange(true)
        }}
      >
        + 新しいノート
      </Button>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent>
          <form onSubmit={onSubmit} className="space-y-4">
            <DialogHeader>
              <DialogTitle>新しいノートを作る</DialogTitle>
              <DialogDescription>
                タイトル、本文、必要に応じて種別を設定します。
              </DialogDescription>
            </DialogHeader>
            <div className="space-y-2">
              <label
                htmlFor="create-note-title"
                className="block font-mono text-2xs uppercase tracking-wide text-muted-foreground"
              >
                タイトル *
              </label>
              <Input
                id="create-note-title"
                autoFocus
                required
                value={title}
                onChange={(e) => {
                  onTitleChange(e.target.value)
                }}
                placeholder="ノートのタイトル"
              />
            </div>
            <div className="space-y-2">
              <label
                htmlFor="create-note-body"
                className="block font-mono text-2xs uppercase tracking-wide text-muted-foreground"
              >
                本文 (Markdown) *
              </label>
              <textarea
                id="create-note-body"
                rows={7}
                required
                value={body}
                onChange={(e) => {
                  onBodyChange(e.target.value)
                }}
                placeholder="本文を入力"
                className="w-full border border-border bg-bg-secondary px-3 py-2 font-mono text-xs text-foreground outline-none focus:border-muted-foreground"
              />
            </div>
            <div className="space-y-2">
              <label
                htmlFor="create-note-kind"
                className="block font-mono text-2xs uppercase tracking-wide text-muted-foreground"
              >
                種別
              </label>
              <select
                id="create-note-kind"
                value={kind}
                onChange={(e) => {
                  onKindChange(e.target.value)
                }}
                disabled={noteKindsPending}
                className="h-9 w-full rounded-md border border-input bg-transparent px-3 font-mono text-xs"
              >
                <option value="">
                  {noteKindsPending ? '読み込み中…' : '種別なし'}
                </option>
                {noteKinds.map((noteKind) => (
                  <option key={noteKind.key} value={noteKind.key}>
                    {noteKind.display_name}
                  </option>
                ))}
              </select>
              {noteKindsError ? (
                <p className="text-xs text-primary">
                  種別一覧を読み込めませんでした
                </p>
              ) : (
                !noteKindsPending &&
                noteKinds.length === 0 && (
                  <p className="text-xs text-muted-foreground">
                    利用できる種別はありません
                  </p>
                )
              )}
            </div>
            {formError != null && (
              <p role="alert" className="text-xs text-primary">
                {formError}
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
              <Button type="submit" disabled={isSubmitting}>
                {isSubmitting ? '作成中…' : '作成'}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </>
  )
}
