import { Button } from '@fohte/ui/button'
import { Input } from '@fohte/ui/input'
import { Textarea } from '@fohte/ui/textarea'
import type { ReactNode, SyntheticEvent } from 'react'
import { useId, useState } from 'react'

export interface AnnotationDraftFields {
  target_kind: string
  text: string
}

export function AnnotationDraftForm({
  ariaLabel,
  header,
  targetKinds,
  isSaving = false,
  errorMessage,
  onSave,
  onCancel,
}: {
  ariaLabel: string
  header: ReactNode
  targetKinds: string[]
  isSaving?: boolean
  errorMessage?: string | null
  onSave: (draft: AnnotationDraftFields) => void
  onCancel: () => void
}) {
  const [targetKind, setTargetKind] = useState('')
  const [text, setText] = useState('')
  const [validationError, setValidationError] = useState<string | null>(null)
  const targetKindsId = useId()

  function handleSubmit(event: SyntheticEvent<HTMLFormElement>) {
    event.preventDefault()
    if (isSaving) return

    const trimmedTargetKind = targetKind.trim()
    const trimmedText = text.trim()
    if (trimmedTargetKind === '' || trimmedText === '') {
      setValidationError('種別キーとメモを入力してください')
      return
    }

    setValidationError(null)
    onSave({ target_kind: trimmedTargetKind, text: trimmedText })
  }

  return (
    <form
      aria-label={ariaLabel}
      className="fixed inset-x-2 bottom-2 z-50 grid gap-3 rounded-lg border border-border bg-card p-3 shadow-xl sm:static sm:z-auto sm:w-72 sm:shrink-0 sm:rounded-none sm:shadow-none"
      onSubmit={handleSubmit}
    >
      {header}
      <label className="grid gap-1 font-mono text-xs text-muted-foreground">
        種別キー
        <Input
          aria-label="種別キー"
          autoComplete="off"
          list={targetKindsId}
          required
          value={targetKind}
          onChange={(event) => {
            setTargetKind(event.currentTarget.value)
          }}
        />
        <datalist id={targetKindsId}>
          {targetKinds.map((kind) => (
            <option key={kind} value={kind} />
          ))}
        </datalist>
      </label>
      <label className="grid gap-1 font-mono text-xs text-muted-foreground">
        メモ
        <Textarea
          aria-label="メモ"
          required
          value={text}
          onChange={(event) => {
            setText(event.currentTarget.value)
          }}
        />
      </label>
      {(validationError ?? errorMessage) != null && (
        <p role="alert" className="text-xs text-destructive">
          {validationError ?? errorMessage}
        </p>
      )}
      <div className="flex justify-end gap-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={isSaving}
          onClick={() => {
            onCancel()
          }}
        >
          取消 (Esc)
        </Button>
        <Button type="submit" size="sm" disabled={isSaving}>
          {isSaving ? '保存中' : '保存'}
        </Button>
      </div>
    </form>
  )
}
