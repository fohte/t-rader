export const buildPhaseMessageText = (input: {
  readonly originalPromptText: string
  readonly phaseKeys: readonly string[]
  readonly phaseKey: string
  readonly phasePrompt: string
  readonly item: unknown
  readonly priorResults: Readonly<Record<string, unknown>>
  readonly inputBindings?: Readonly<Record<string, unknown>>
}): string => {
  const phaseScopeMessage = `この巡回は ${input.phaseKeys.join(' → ')} の順に進む。あなたは ${input.phaseKey} を担当する。依頼のうち他の phase の担当分は、後の phase が行う`
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
  if (
    input.inputBindings !== undefined &&
    Object.keys(input.inputBindings).length > 0
  ) {
    sections.push(
      `このフェーズに束縛された入力:\n\`\`\`json\n${JSON.stringify(input.inputBindings, null, 2)}\n\`\`\``,
    )
  }
  if (Object.keys(input.priorResults).length > 0) {
    sections.push(
      `これまでのフェーズの結果:\n\`\`\`json\n${JSON.stringify(input.priorResults, null, 2)}\n\`\`\``,
    )
  }
  return sections.join('\n\n---\n\n')
}
