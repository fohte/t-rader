import type { RefObject } from 'react'

import {
  addPhase,
  DEFAULT_AGENT_GRAPH_YAML,
  movePhase,
  parseAgentGraphPhases,
  removePhase,
  setPhaseArrayField,
  setPhaseField,
} from '#components/strategy-settings/agent-graph/document'
import { ModelField } from '#components/strategy-settings/agent-graph/fields/model-field'
import { PromptField } from '#components/strategy-settings/agent-graph/fields/prompt-field'
import { SkillsField } from '#components/strategy-settings/agent-graph/fields/skills-field'
import { ToolsField } from '#components/strategy-settings/agent-graph/fields/tools-field'
import { PhaseCard } from '#components/strategy-settings/agent-graph/phase-card'
import { $api } from '#lib/api/client'

interface AgentGraphFormProps {
  strategyId: string
  /** agent_graph の YAML 文字列。これが唯一の真実の情報源で、フォーム操作のたびに書き換える */
  value: string
  onChange: (next: string) => void
  /** 保存失敗時、原因になったフェーズの key (save-error.ts で抽出したもの) */
  errorPhaseKey?: string | null
  /**
   * フェーズ分割 off (空文字列) にする直前の内容。on に戻したとき復元する。
   * ビュー切替 (フォーム ⇔ YAML) を跨いでも失われないよう、呼び出し側 (AgentGraphEditor) に
   * 保持させる
   */
  lastEnabledValueRef: RefObject<string>
}

export function AgentGraphForm({
  strategyId,
  value,
  onChange,
  errorPhaseKey = null,
  lastEnabledValueRef,
}: AgentGraphFormProps) {
  const enabled = value.trim() !== ''

  const { data: modelsData } = $api.useQuery('get', '/api/agent-models')
  const { data: toolsData } = $api.useQuery('get', '/api/agent-tools')
  const { data: skillsData } = $api.useQuery(
    'get',
    '/api/strategies/{id}/skills',
    { params: { path: { id: strategyId } } },
  )
  const models = modelsData?.models ?? []
  const tools = toolsData?.tools ?? []
  const skillNames = Object.keys(skillsData?.skills ?? {}).sort()

  const phases = parseAgentGraphPhases(value) ?? []
  const labelByKey = new Map(phases.map((p) => [p.key, p.label]))

  function handleToggle(next: boolean) {
    if (!next) {
      onChange('')
      return
    }
    onChange(
      lastEnabledValueRef.current.trim() !== ''
        ? lastEnabledValueRef.current
        : DEFAULT_AGENT_GRAPH_YAML,
    )
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-3">
        <label className="flex items-center gap-1.5 font-mono text-[11.5px] text-[color:var(--color-text-secondary)]">
          <input
            type="checkbox"
            aria-label="フェーズ分割を有効にする"
            checked={enabled}
            onChange={(e) => {
              handleToggle(e.target.checked)
            }}
          />
          フェーズ分割を有効にする
        </label>
        <span className="font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
          off にすると単一フェーズ (現行の挙動) で実行されます
        </span>
      </div>

      {enabled && (
        <div className="space-y-2">
          {phases.map((phase, i) => (
            <PhaseCard
              key={phase.key}
              index={i}
              total={phases.length}
              phase={phase}
              referencedLabel={
                phase.forEach != null
                  ? labelByKey.get(phase.forEach.split('.')[0] ?? '')
                  : undefined
              }
              hasError={phase.key === errorPhaseKey}
              onMoveUp={() => {
                onChange(movePhase(value, i, 'up'))
              }}
              onMoveDown={() => {
                onChange(movePhase(value, i, 'down'))
              }}
              onRemove={() => {
                onChange(removePhase(value, i))
              }}
            >
              <ModelField
                value={phase.model}
                onChange={(next) => {
                  onChange(setPhaseField(value, i, 'model', next))
                }}
                models={models}
              />
              <PromptField
                value={phase.prompt}
                onChange={(next) => {
                  onChange(setPhaseField(value, i, 'prompt', next))
                }}
              />
              <ToolsField
                value={phase.tools}
                onChange={(next) => {
                  onChange(setPhaseArrayField(value, i, 'tools', next))
                }}
                options={tools}
              />
              <SkillsField
                value={phase.skills}
                onChange={(next) => {
                  onChange(setPhaseArrayField(value, i, 'skills', next))
                }}
                options={skillNames}
              />
            </PhaseCard>
          ))}
          <button
            type="button"
            onClick={() => {
              onChange(addPhase(value))
            }}
            className="w-full border border-dashed border-[color:var(--color-border-strategy)] py-1.5 font-mono text-[12px] text-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-text-primary)]"
          >
            + フェーズを追加
          </button>
        </div>
      )}
    </div>
  )
}
