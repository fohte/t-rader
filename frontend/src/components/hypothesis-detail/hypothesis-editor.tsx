import { useEffect, useRef, useState } from 'react'

import { MarkdownBody } from '@/components/note-detail/markdown-body'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

interface HypothesisEditorProps {
  initialTitle: string
  initialBody: string
  onSave: (next: { title: string; body: string }) => void
  isSaving?: boolean
  saveError?: string | null
}

export function HypothesisEditor({
  initialTitle,
  initialBody,
  onSave,
  isSaving = false,
  saveError = null,
}: HypothesisEditorProps) {
  const [title, setTitle] = useState(initialTitle)
  const [body, setBody] = useState(initialBody)
  const [validationError, setValidationError] = useState<string | null>(null)
  // フィールドごとに「直前に親から受け取った初期値」を保持し、
  // 未編集 (前回の初期値のまま) の場合のみ外部更新を追従させる。
  // 編集中に別の PATCH (status 切替等) が detail を再取得しても、この場合はドラフトを消さない
  const lastInitialTitleRef = useRef(initialTitle)
  const lastInitialBodyRef = useRef(initialBody)
  const dirty = title !== initialTitle || body !== initialBody

  useEffect(() => {
    if (title === lastInitialTitleRef.current) {
      setTitle(initialTitle)
    }
    lastInitialTitleRef.current = initialTitle
  }, [initialTitle, title])

  useEffect(() => {
    if (body === lastInitialBodyRef.current) {
      setBody(initialBody)
    }
    lastInitialBodyRef.current = initialBody
  }, [initialBody, body])

  function handleSave() {
    if (!dirty || isSaving) return
    if (title.trim() === '' || body.trim() === '') {
      setValidationError('title と body は必須です')
      return
    }
    setValidationError(null)
    onSave({ title, body })
  }

  return (
    <div className="space-y-3">
      <div className="space-y-1.5">
        <label
          htmlFor="hypothesis-title-input"
          className="block font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]"
        >
          title
        </label>
        <Input
          id="hypothesis-title-input"
          value={title}
          onChange={(e) => {
            setTitle(e.target.value)
          }}
        />
      </div>
      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        <div className="space-y-1.5">
          <label
            htmlFor="hypothesis-body-source"
            className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]"
          >
            body
          </label>
          <textarea
            id="hypothesis-body-source"
            value={body}
            onChange={(e) => {
              setBody(e.target.value)
            }}
            rows={10}
            className="w-full resize-y border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-secondary)] p-3 font-mono text-[12.5px] leading-relaxed text-[color:var(--color-text-primary)] outline-none focus:border-[color:var(--color-text-tertiary)]"
          />
        </div>
        <div className="space-y-1.5">
          <span className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
            preview
          </span>
          <div
            data-testid="hypothesis-body-preview"
            style={{ minHeight: '240px' }}
            className="overflow-auto border border-[color:var(--color-hairline)] bg-[color:var(--color-bg-secondary)] px-4 py-2"
          >
            <MarkdownBody source={body} />
          </div>
        </div>
      </div>
      <div className="flex items-center gap-3">
        <Button
          type="button"
          onClick={handleSave}
          disabled={!dirty || isSaving}
        >
          {isSaving ? '保存中…' : '保存'}
        </Button>
        {(validationError != null || saveError != null) && (
          <span
            data-testid="hypothesis-editor-error"
            className="font-mono text-[12px] text-[color:var(--color-accent-strategy)]"
          >
            {validationError ?? saveError}
          </span>
        )}
      </div>
    </div>
  )
}
