import { Button } from '@fohte/ui/button'
import { Chip } from '@fohte/ui/chip'
import { useEffect, useRef, useState } from 'react'

import { CodeEditor } from '#components/indicators/code-editor'
import { AgentGraphForm } from '#components/strategy-settings/agent-graph/agent-graph-form'
import { parseAgentGraphPhases } from '#components/strategy-settings/agent-graph/document'
import { extractPhaseKeyFromSaveError } from '#components/strategy-settings/agent-graph/save-error'
import {
  type AgentGraphView,
  AgentGraphViewSelector,
} from '#components/strategy-settings/agent-graph/view-selector'

interface AgentGraphEditorProps {
  purpose: string
  /** 永続化されている現在の内容 (保存ボタン押下時の diff 元) */
  initialValue: string
  /** 保存ハンドラ。エラー表示は呼び出し側で saveError prop 経由に倒す */
  onSave: (next: string) => void
  isSaving?: boolean
  saveError?: string | null
}

export function AgentGraphEditor({
  purpose,
  initialValue,
  onSave,
  isSaving = false,
  saveError = null,
}: AgentGraphEditorProps) {
  const [value, setValue] = useState(initialValue)
  const [view, setView] = useState<AgentGraphView>('form')
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
  const effectiveView: AgentGraphView = formAvailable ? view : 'yaml'
  const errorPhaseKey =
    saveError != null ? extractPhaseKeyFromSaveError(saveError) : null

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <Chip>agent_graph</Chip>
        <div className="flex items-center gap-3">
          {dirty && (
            <Chip data-testid="dirty-indicator" tone="strong">
              未保存の変更あり
            </Chip>
          )}
          <AgentGraphViewSelector
            value={effectiveView}
            formAvailable={formAvailable}
            onChange={setView}
          />
        </div>
      </div>

      {!formAvailable && (
        <p
          data-testid="form-unavailable-notice"
          className="font-mono text-2xs text-muted-foreground"
        >
          YAML の構文が壊れているためフォーム表示に切り替えられません。YAML
          を直接修正してください。
        </p>
      )}

      {effectiveView === 'form' ? (
        <AgentGraphForm
          purpose={purpose}
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
            className="whitespace-pre-wrap font-mono text-xs text-primary"
          >
            {saveError}
          </span>
        )}
      </div>
    </div>
  )
}
