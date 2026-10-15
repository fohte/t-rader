import { Button } from '@fohte/ui/button'
import { Input } from '@fohte/ui/input'
import { ChevronDown, Pencil, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '#components/ui/dialog'
import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'
import type { SavedScreen } from '#lib/screener-search'

export type SavedScreenDialogState =
  | { type: 'save' }
  | { type: 'rename'; savedScreen: SavedScreen }
  | { type: 'delete'; savedScreen: SavedScreen }

export type SaveDialogState = Extract<
  SavedScreenDialogState,
  { type: 'save' | 'rename' }
>

export function SavedScreensControlView({
  savedScreens,
  selectedScreen,
  selectedScreenId,
  selectedScreenHasChanges = false,
  menuOpen,
  dialog,
  isPending = false,
  error,
  savedScreensPending = false,
  listError = false,
  onMenuOpenChange,
  onDialogChange,
  onSelect,
  onSave,
  onOverwrite,
  onDelete,
}: {
  savedScreens: SavedScreen[]
  selectedScreen: SavedScreen | undefined
  selectedScreenId?: string
  selectedScreenHasChanges?: boolean
  menuOpen: boolean
  dialog: SavedScreenDialogState | null
  isPending?: boolean
  error?: string
  savedScreensPending?: boolean
  listError?: boolean
  onMenuOpenChange: (open: boolean) => void
  onDialogChange: (dialog: SavedScreenDialogState | null) => void
  onSelect: (savedScreen: SavedScreen) => void
  onSave: (name: string, dialog: SaveDialogState) => void
  onOverwrite: () => void
  onDelete: (savedScreen: SavedScreen) => void
}) {
  const saveDialog =
    dialog?.type === 'save' || dialog?.type === 'rename' ? dialog : null
  const saveActionDisabled = isPending || savedScreensPending
  const [name, setName] = useState('')

  useEffect(() => {
    setName(saveDialog?.type === 'rename' ? saveDialog.savedScreen.name : '')
  }, [
    saveDialog?.type,
    saveDialog?.type === 'rename' ? saveDialog.savedScreen.id : null,
  ])

  function handleSaveSubmit(event: React.SyntheticEvent<HTMLFormElement>) {
    event.preventDefault()
    event.stopPropagation()
    const trimmedName = name.trim()
    if (saveDialog == null || trimmedName === '' || isPending) return
    onSave(trimmedName, saveDialog)
  }

  function renderSaveActions() {
    return (
      <div className="flex flex-col gap-1 border-t border-border pt-2">
        {selectedScreen != null && selectedScreenHasChanges && (
          <button
            type="button"
            disabled={saveActionDisabled}
            onClick={() => {
              onOverwrite()
              onMenuOpenChange(false)
            }}
            className="w-full px-2 py-2 text-left text-2xs text-muted-foreground-strong hover:bg-surface-strong hover:text-foreground disabled:opacity-50"
          >
            上書き保存
          </button>
        )}
        <button
          type="button"
          disabled={saveActionDisabled}
          onClick={() => {
            onDialogChange({ type: 'save' })
            onMenuOpenChange(false)
          }}
          className="w-full px-2 py-2 text-left text-2xs text-muted-foreground-strong hover:bg-surface-strong hover:text-foreground disabled:opacity-50"
        >
          {savedScreensPending
            ? '読み込み中...'
            : selectedScreen == null
              ? 'この条件を保存'
              : '名前を付けて保存'}
        </button>
      </div>
    )
  }

  return (
    <>
      <div className="flex min-w-0 flex-col gap-1">
        <div className="flex min-w-0 items-center gap-2">
          <Popover open={menuOpen} onOpenChange={onMenuOpenChange}>
            <PopoverTrigger
              render={
                <button
                  type="button"
                  aria-label={
                    selectedScreen == null
                      ? savedScreensPending && selectedScreenId != null
                        ? '保存した条件を読み込み中'
                        : '保存した条件を選ぶ'
                      : `保存した条件: ${selectedScreen.name}`
                  }
                  className="inline-flex min-w-0 items-center gap-1.5 border border-border bg-background px-2.5 py-2 text-left text-2xs text-muted-foreground-strong hover:border-muted-foreground hover:text-foreground"
                >
                  <span className="min-w-0 truncate">
                    {selectedScreen == null ? (
                      savedScreensPending && selectedScreenId != null ? (
                        '読み込み中...'
                      ) : (
                        '保存した条件'
                      )
                    ) : (
                      <>
                        <span className="hidden md:inline">保存した条件: </span>
                        {selectedScreen.name}
                      </>
                    )}
                  </span>
                  <ChevronDown
                    aria-hidden="true"
                    className="size-3.5 shrink-0"
                  />
                </button>
              }
            />
            <PopoverContent
              align="start"
              sideOffset={6}
              className="w-80 space-y-2 p-2"
              style={{ maxWidth: 'calc(100vw - 1.5rem)' }}
            >
              <h3 className="px-2 py-1 font-mono text-2xs font-semibold text-muted-foreground">
                保存した条件
              </h3>
              {savedScreensPending ? (
                <p className="px-2 py-3 text-2xs text-muted-foreground">
                  読み込み中...
                </p>
              ) : listError ? (
                <p role="alert" className="px-2 py-3 text-2xs text-destructive">
                  保存した条件を読み込めませんでした。
                </p>
              ) : savedScreens.length === 0 ? (
                <p className="px-2 py-3 text-2xs text-muted-foreground">
                  保存した条件はありません。
                </p>
              ) : (
                <ul className="max-h-64 space-y-0.5 overflow-y-auto">
                  {savedScreens.map((savedScreen) => (
                    <li
                      key={savedScreen.id}
                      className="flex items-center gap-1"
                    >
                      <button
                        type="button"
                        aria-current={
                          selectedScreenId === savedScreen.id
                            ? 'true'
                            : undefined
                        }
                        onClick={() => {
                          onSelect(savedScreen)
                          onMenuOpenChange(false)
                        }}
                        className={`min-w-0 flex-1 truncate px-2 py-2 text-left text-2xs hover:bg-surface-strong ${selectedScreenId === savedScreen.id ? 'bg-surface-strong text-foreground' : 'text-muted-foreground-strong'}`}
                      >
                        {savedScreen.name}
                      </button>
                      <button
                        type="button"
                        aria-label={`${savedScreen.name} の名前を変更`}
                        onClick={() => {
                          onDialogChange({ type: 'rename', savedScreen })
                          onMenuOpenChange(false)
                        }}
                        className="inline-flex size-8 shrink-0 items-center justify-center text-muted-foreground hover:bg-surface-strong hover:text-foreground"
                      >
                        <Pencil aria-hidden="true" className="size-3.5" />
                      </button>
                      <button
                        type="button"
                        aria-label={`${savedScreen.name} を削除`}
                        onClick={() => {
                          onDialogChange({ type: 'delete', savedScreen })
                          onMenuOpenChange(false)
                        }}
                        className="inline-flex size-8 shrink-0 items-center justify-center text-muted-foreground hover:bg-surface-strong hover:text-destructive"
                      >
                        <Trash2 aria-hidden="true" className="size-3.5" />
                      </button>
                    </li>
                  ))}
                </ul>
              )}
              <div className="md:hidden">{renderSaveActions()}</div>
            </PopoverContent>
          </Popover>
          <div className="hidden shrink-0 items-center gap-2 md:flex">
            {selectedScreen != null && selectedScreenHasChanges && (
              <Button
                type="button"
                variant="outline"
                className="text-2xs"
                disabled={saveActionDisabled}
                onClick={onOverwrite}
              >
                上書き保存
              </Button>
            )}
            <Button
              type="button"
              variant="outline"
              className="text-2xs"
              disabled={saveActionDisabled}
              onClick={() => {
                onDialogChange({ type: 'save' })
              }}
            >
              {savedScreensPending
                ? '読み込み中...'
                : selectedScreen == null
                  ? '保存'
                  : '名前を付けて保存'}
            </Button>
          </div>
        </div>
        {error != null && dialog == null && (
          <p role="alert" className="text-2xs text-destructive">
            {error}
          </p>
        )}
      </div>

      <Dialog
        open={dialog != null}
        onOpenChange={(open) => {
          if (!open) onDialogChange(null)
        }}
      >
        {saveDialog != null && (
          <DialogContent>
            <form onSubmit={handleSaveSubmit} className="space-y-4">
              <DialogHeader>
                <DialogTitle>
                  {saveDialog.type === 'rename'
                    ? '保存した条件の名前を変更'
                    : '条件に名前を付けて保存'}
                </DialogTitle>
                <DialogDescription>
                  条件と並び順を名前付きで保存します。
                </DialogDescription>
              </DialogHeader>
              <div className="space-y-2">
                <label
                  htmlFor="saved-screen-name"
                  className="block font-mono text-2xs text-muted-foreground"
                >
                  条件名
                </label>
                <Input
                  id="saved-screen-name"
                  required
                  autoFocus
                  value={name}
                  onChange={(event) => {
                    setName(event.currentTarget.value)
                  }}
                  placeholder="例: 注目条件"
                />
              </div>
              {error != null && (
                <p role="alert" className="text-xs text-destructive">
                  {error}
                </p>
              )}
              <DialogFooter>
                <Button
                  type="button"
                  variant="outline"
                  disabled={isPending}
                  onClick={() => {
                    onDialogChange(null)
                  }}
                >
                  キャンセル
                </Button>
                <Button
                  type="submit"
                  disabled={name.trim() === '' || isPending}
                >
                  {isPending
                    ? '保存中...'
                    : saveDialog.type === 'rename'
                      ? '名前を変更'
                      : '保存'}
                </Button>
              </DialogFooter>
            </form>
          </DialogContent>
        )}
        {dialog?.type === 'delete' && (
          <DialogContent>
            <DialogHeader>
              <DialogTitle>保存した条件を削除</DialogTitle>
              <DialogDescription>
                「{dialog.savedScreen.name}
                」を削除します。この操作は取り消せません。
              </DialogDescription>
            </DialogHeader>
            {error != null && (
              <p role="alert" className="text-xs text-destructive">
                {error}
              </p>
            )}
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                disabled={isPending}
                onClick={() => {
                  onDialogChange(null)
                }}
              >
                キャンセル
              </Button>
              <Button
                type="button"
                variant="destructive"
                disabled={isPending}
                onClick={() => {
                  onDelete(dialog.savedScreen)
                }}
              >
                {isPending ? '削除中...' : '削除'}
              </Button>
            </DialogFooter>
          </DialogContent>
        )}
      </Dialog>
    </>
  )
}
