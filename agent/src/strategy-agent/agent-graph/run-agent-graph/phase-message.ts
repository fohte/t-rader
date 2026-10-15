export const buildPhaseMessageText = (input: {
  readonly originalPromptText: string
  readonly phaseKeys: readonly string[]
  readonly phaseKey: string
  readonly phasePrompt: string
  readonly item: unknown
  readonly priorResults: Readonly<Record<string, unknown>>
}): string => {
  const currentPhaseIndex = input.phaseKeys.indexOf(input.phaseKey)
  const phaseScopeParts = [
    `この巡回は ${input.phaseKeys.join(' → ')} の順に進む。あなたは ${input.phaseKey} を担当する。`,
  ]
  if (currentPhaseIndex > 0) {
    phaseScopeParts.push('前の phase の担当分は完了済み。')
  }
  if (currentPhaseIndex < input.phaseKeys.length - 1) {
    phaseScopeParts.push(
      currentPhaseIndex === 0
        ? '依頼のうち他の phase の担当分は、後の phase が行う。'
        : '依頼のうち後の phase の担当分は、後の phase が行う。',
    )
  } else if (currentPhaseIndex === 0) {
    phaseScopeParts.push('依頼全体をこの phase で処理する。')
  } else {
    phaseScopeParts.push('依頼のうち他の phase の担当分はすべて完了済み。')
  }
  const phaseScopeMessage = phaseScopeParts.join('')
  const sections = [
    input.originalPromptText,
    phaseScopeMessage,
    input.phasePrompt,
  ]
  if (input.item !== undefined) {
    sections.push(
      `割り当てられた対象:\n\`\`\`json\n${JSON.stringify(input.item, null, 2)}\n\`\`\``,
    )
  }
  if (Object.keys(input.priorResults).length > 0) {
    sections.push(
      `これまでのフェーズの結果:\n\`\`\`json\n${JSON.stringify(input.priorResults, null, 2)}\n\`\`\``,
    )
  }
  return sections.join('\n\n---\n\n')
}
