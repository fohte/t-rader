import { Button } from '@fohte/ui/button'
import { Input } from '@fohte/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@fohte/ui/select'
import { Textarea } from '@fohte/ui/textarea'

import {
  decodePrefixedSelectValue,
  encodePrefixedSelectValue,
} from '#components/strategy-home/prefixed-select-value'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '#components/ui/dialog'
import type { components } from '#lib/api/schema.gen'

type NoteKind = components['schemas']['NoteKind']

const NO_KIND_VALUE = '__no-kind__'
const LOADING_KIND_VALUE = '__loading-kind__'
const KIND_VALUE_PREFIX = 'kind:'
const NO_KIND_LABEL = '種別なし'
const LOADING_KIND_LABEL = '読み込み中…'

function getSelectedKindLabel(
  kind: string,
  noteKinds: NoteKind[],
  noteKindsPending: boolean,
) {
  if (kind === '') {
    return noteKindsPending ? LOADING_KIND_LABEL : NO_KIND_LABEL
  }

  return (
    noteKinds.find((noteKind) => noteKind.key === kind)?.display_name ?? kind
  )
}

export interface CreateNoteDialogViewProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  showTrigger?: boolean
  title: string
  body: string
  kind: string
  noteKinds: NoteKind[]
  noteKindsPending: boolean
  noteKindsError: boolean
  kindSelectDefaultOpen?: boolean
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
  showTrigger = true,
  title,
  body,
  kind,
  noteKinds,
  noteKindsPending,
  noteKindsError,
  kindSelectDefaultOpen,
  formError,
  isSubmitting,
  onTitleChange,
  onBodyChange,
  onKindChange,
  onSubmit,
}: CreateNoteDialogViewProps) {
  const selectedKindLabel = getSelectedKindLabel(
    kind,
    noteKinds,
    noteKindsPending,
  )

  return (
    <>
      {showTrigger && (
        <Button
          onClick={() => {
            onOpenChange(true)
          }}
        >
          + 新しいノート
        </Button>
      )}
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
              <Textarea
                id="create-note-body"
                rows={7}
                required
                value={body}
                onChange={(e) => {
                  onBodyChange(e.target.value)
                }}
                placeholder="本文を入力"
              />
            </div>
            <div className="space-y-2">
              <label
                htmlFor="create-note-kind"
                className="block font-mono text-2xs uppercase tracking-wide text-muted-foreground"
              >
                種別
              </label>
              <Select
                value={encodePrefixedSelectValue(
                  kind,
                  KIND_VALUE_PREFIX,
                  noteKindsPending ? LOADING_KIND_VALUE : NO_KIND_VALUE,
                )}
                defaultOpen={kindSelectDefaultOpen}
                onValueChange={(value) => {
                  onKindChange(
                    decodePrefixedSelectValue(value, KIND_VALUE_PREFIX, [
                      NO_KIND_VALUE,
                      LOADING_KIND_VALUE,
                    ]) ?? '',
                  )
                }}
              >
                <SelectTrigger
                  id="create-note-kind"
                  disabled={noteKindsPending}
                >
                  <SelectValue>{selectedKindLabel}</SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {noteKindsPending ? (
                    <SelectItem value={LOADING_KIND_VALUE} disabled>
                      {LOADING_KIND_LABEL}
                    </SelectItem>
                  ) : (
                    <SelectItem value={NO_KIND_VALUE}>
                      {NO_KIND_LABEL}
                    </SelectItem>
                  )}
                  {noteKinds.map((noteKind) => (
                    <SelectItem
                      key={noteKind.key}
                      value={encodePrefixedSelectValue(
                        noteKind.key,
                        KIND_VALUE_PREFIX,
                        NO_KIND_VALUE,
                      )}
                    >
                      {noteKind.display_name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {noteKindsError ? (
                <p className="text-xs text-destructive">
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
              <p role="alert" className="text-xs text-destructive">
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
