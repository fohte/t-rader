import { useRef, useState } from 'react'

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

  // レンダー中に prop の変化を検知し、未編集 (前回の初期値のまま) の場合のみ同期する。
  // useEffect にすると、新規オブジェクトを毎レンダー生成して依存配列に積む形になり、
  // draft を初期値へ同期するたびに参照が変わって再レンダー→同期→再レンダー…と
  // 無限ループするため、prop の文字列そのものを ref と比較してレンダー中に同期する
  const prevInitialTitleRef = useRef(initialTitle)
  const prevInitialBodyRef = useRef(initialBody)
  if (prevInitialTitleRef.current !== initialTitle) {
    if (title === prevInitialTitleRef.current) {
      setTitle(initialTitle)
    }
    prevInitialTitleRef.current = initialTitle
  }
  if (prevInitialBodyRef.current !== initialBody) {
    if (body === prevInitialBodyRef.current) {
      setBody(initialBody)
    }
    prevInitialBodyRef.current = initialBody
  }

  const dirty = title !== initialTitle || body !== initialBody

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
