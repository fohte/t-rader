import { useEffect, useRef, useState } from 'react'

import { CodeEditor } from '#components/indicators/code-editor'
import { AgentGraphForm } from '#components/strategy-settings/agent-graph/agent-graph-form'
import { parseAgentGraphPhases } from '#components/strategy-settings/agent-graph/document'
import { extractPhaseKeyFromSaveError } from '#components/strategy-settings/agent-graph/save-error'
import { Button } from '#components/ui/button'
import { cn } from '#lib/utils'

interface AgentGraphEditorProps {
  strategyId: string
  /** 永続化されている現在の内容 (保存ボタン押下時の diff 元) */
  initialValue: string
  /** 保存ハンドラ。エラー表示は呼び出し側で saveError prop 経由に倒す */
  onSave: (next: string) => void
  isSaving?: boolean
  saveError?: string | null
}

export function AgentGraphEditor({
  strategyId,
  initialValue,
  onSave,
  isSaving = false,
  saveError = null,
}: AgentGraphEditorProps) {
  const [value, setValue] = useState(initialValue)
  const [view, setView] = useState<'form' | 'yaml'>('form')
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

  // フェーズ分割 off (空文字列) にする直前の内容を覚えておき、on に戻したとき復元する。
  // AgentGraphForm 側に持たせるとフォーム/YAML ビュー切替のたびにアンマウントされて失われるため、
  // ビュー切替を跨いで生き続けるこのコンポーネントで保持する
  const lastEnabledValueRef = useRef(value)
  useEffect(() => {
    if (value.trim() !== '') lastEnabledValueRef.current = value
  }, [value])

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

  // 構文が壊れている、またはフェーズが key/label/model/prompt を string で持たない場合は
  // フォームで表示できないので YAML ビューに固定する
  const formAvailable = parseAgentGraphPhases(value) != null
  const effectiveView = formAvailable ? view : 'yaml'
  const errorPhaseKey =
    saveError != null ? extractPhaseKeyFromSaveError(saveError) : null

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <label className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
          agent_graph
        </label>
        <div className="flex items-center gap-3">
          {dirty && (
            <span
              data-testid="dirty-indicator"
              className="font-mono text-[11px] text-[color:var(--color-accent-strategy)]"
            >
              未保存の変更あり
            </span>
          )}
          <div className="flex gap-0.5">
            <button
              type="button"
              onClick={() => {
                setView('form')
              }}
              disabled={!formAvailable}
              aria-pressed={effectiveView === 'form'}
              className={viewChipClass(effectiveView === 'form')}
            >
              フォーム
            </button>
            <button
              type="button"
              onClick={() => {
                setView('yaml')
              }}
              aria-pressed={effectiveView === 'yaml'}
              className={viewChipClass(effectiveView === 'yaml')}
            >
              YAML
            </button>
          </div>
        </div>
      </div>

      {!formAvailable && (
        <p
          data-testid="form-unavailable-notice"
          className="font-mono text-[11px] text-[color:var(--color-text-tertiary)]"
        >
          YAML の構文が壊れているためフォーム表示に切り替えられません。YAML
          を直接修正してください。
        </p>
      )}

      {effectiveView === 'form' ? (
        <AgentGraphForm
          strategyId={strategyId}
          value={value}
          onChange={setValue}
          errorPhaseKey={errorPhaseKey}
          lastEnabledValueRef={lastEnabledValueRef}
        />
      ) : (
        <CodeEditor
          language="yaml"
          value={value}
          onChange={setValue}
          testId="agent-graph-editor"
          ariaLabel="agent_graph"
          height={480}
        />
      )}

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

function viewChipClass(active: boolean) {
  return cn(
    'border px-2 py-0.5 font-mono text-[10.5px]',
    active
      ? 'border-[color:var(--color-accent-strategy)] bg-[color:var(--color-bg-tertiary)] text-[color:var(--color-accent-strategy)]'
      : 'border-[color:var(--color-border-strategy)] text-[color:var(--color-text-tertiary)] disabled:opacity-30',
  )
}
