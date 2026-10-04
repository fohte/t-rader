export const buildPhaseMessageText = (input: {
  readonly originalPromptText: string
  readonly phasePrompt: string
  readonly item: unknown
  readonly priorResults: Readonly<Record<string, unknown>>
}): string => {
  const sections = [input.originalPromptText, input.phasePrompt]
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
