import { useEffect, useRef, useState } from 'react'

import { CodeEditor } from '#components/indicators/code-editor'
import { Button } from '#components/ui/button'

interface AgentGraphEditorProps {
  /** 永続化されている現在の内容 (保存ボタン押下時の diff 元) */
  initialValue: string
  /** 保存ハンドラ。エラー表示は呼び出し側で saveError prop 経由に倒す */
  onSave: (next: string) => void
  isSaving?: boolean
  saveError?: string | null
}

export function AgentGraphEditor({
  initialValue,
  onSave,
  isSaving = false,
  saveError = null,
}: AgentGraphEditorProps) {
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
      <div className="flex items-center justify-between">
        <label className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
          agent_graph (YAML)
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
      <CodeEditor
        language="yaml"
        value={value}
        onChange={setValue}
        testId="agent-graph-editor"
        ariaLabel="agent_graph"
        height={480}
      />
      <div className="flex items-center gap-3">
        <Button
          type="button"
          onClick={handleSave}
          disabled={!dirty || isSaving}
        >
          {isSaving ? '保存中…' : '保存'}
        </Button>
        {saveError != null && (
          <span
            data-testid="save-error"
            className="whitespace-pre-wrap font-mono text-[12px] text-[color:var(--color-accent-strategy)]"
          >
            {saveError}
          </span>
        )}
      </div>
    </div>
  )
}
