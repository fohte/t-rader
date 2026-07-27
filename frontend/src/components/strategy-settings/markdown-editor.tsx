import { useEffect, useRef, useState } from 'react'

import { MarkdownBody } from '#components/note-detail/markdown-body'
import { Button } from '#components/ui/button'

interface MarkdownEditorProps {
  /** 永続化されている現在の内容 (保存ボタン押下時の diff 元) */
  initialValue: string
  /** 保存ハンドラ。エラー表示は呼び出し側で saveError prop 経由に倒す */
  onSave: (next: string) => void
  isSaving?: boolean
  saveError?: string | null
  /** エディタ本体の最小高さ */
  minHeight?: number
}

export function MarkdownEditor({
  initialValue,
  onSave,
  isSaving = false,
  saveError = null,
  minHeight = 320,
}: MarkdownEditorProps) {
  const [value, setValue] = useState(initialValue)
  // 前回親から受け取った initialValue。これと value が一致していれば「ユーザー未編集」と判定できる。
  // dirty (= value !== initialValue) で判定すると、initialValue 変化と同 render で dirty=true になり
  // 「クリーンなのに追従しない」状態に陥るので別に保持する
  const lastInitialValueRef = useRef(initialValue)
  const dirty = value !== initialValue

  useEffect(() => {
    if (value === lastInitialValueRef.current) {
      setValue(initialValue)
    }
    lastInitialValueRef.current = initialValue
  }, [initialValue, value])

  useEffect(() => {
    if (!dirty) return
    function handler(e: BeforeUnloadEvent) {
      e.preventDefault()
    }
    window.addEventListener('beforeunload', handler)
    return () => {
      window.removeEventListener('beforeunload', handler)
    }
  }, [dirty])

  function handleSave() {
    if (!dirty || isSaving) return
    onSave(value)
  }

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        <div className="space-y-1.5">
          <div className="flex items-center justify-between">
            <label
              htmlFor="markdown-source"
              className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]"
            >
              source
            </label>
            {dirty && (
              <span
                data-testid="dirty-indicator"
                className="font-mono text-[11px] text-[color:var(--color-accent-strategy)]"
              >
                未保存の変更あり
              </span>
            )}
          </div>
          <textarea
            id="markdown-source"
            value={value}
            onChange={(e) => {
              setValue(e.target.value)
            }}
            style={{ minHeight: `${String(minHeight)}px` }}
            className="w-full resize-y border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-secondary)] p-3 font-mono text-[12.5px] leading-relaxed text-[color:var(--color-text-primary)] outline-none focus:border-[color:var(--color-text-tertiary)]"
          />
        </div>
        <div className="space-y-1.5">
          <div className="flex items-center justify-between">
            <span className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
              preview
            </span>
          </div>
          <div
            data-testid="markdown-preview"
            style={{ minHeight: `${String(minHeight)}px` }}
            className="overflow-auto border border-[color:var(--color-hairline)] bg-[color:var(--color-bg-secondary)] px-4 py-2"
          >
            <MarkdownBody source={value} />
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
        {saveError != null && (
          <span className="font-mono text-[12px] text-[color:var(--color-accent-strategy)]">
            {saveError}
          </span>
        )}
      </div>
    </div>
  )
}
