import { Button } from '@fohte/ui/button'
import { type SyntheticEvent, useId, useState } from 'react'

import type { HorizontalLineAnnotationDraft } from '#lib/annotation-chart-utils'

export function HorizontalLineEditor({
  timestamp,
  price,
  targetKinds,
  isSaving = false,
  errorMessage,
  onSave,
  onCancel,
}: {
  timestamp: string
  price: number
  targetKinds: string[]
  isSaving?: boolean
  errorMessage?: string | null
  onSave: (draft: HorizontalLineAnnotationDraft) => void
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
    onSave({
      target_kind: trimmedTargetKind,
      text: trimmedText,
      timestamp,
      price,
    })
  }

  return (
    <form
      aria-label="水平線のアノテーション"
      className="fixed inset-x-2 bottom-2 z-50 grid gap-3 rounded-lg border border-border bg-card p-3 shadow-xl sm:static sm:z-auto sm:w-72 sm:shrink-0 sm:rounded-none sm:shadow-none"
      onSubmit={handleSubmit}
    >
      <div className="flex items-baseline justify-between gap-2">
        <h2 className="font-mono text-xs font-bold text-foreground">
          新しい水平線
        </h2>
        <span className="font-mono text-2xs text-muted-foreground">
          {new Date(timestamp).toLocaleDateString('ja-JP')} ·{' '}
          {price.toLocaleString('ja-JP')}
        </span>
      </div>
      <label className="grid gap-1 font-mono text-xs text-muted-foreground">
        種別キー
        <input
          aria-label="種別キー"
          autoComplete="off"
          className="h-9 min-w-0 border border-border bg-background px-2 text-foreground outline-none focus:border-primary"
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
        <textarea
          aria-label="メモ"
          className="min-h-20 resize-y border border-border bg-background p-2 font-sans text-sm text-foreground outline-none focus:border-primary"
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
