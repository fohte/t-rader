// 実行の基準時刻を LLM に渡すためにプロンプトの先頭へ付与する。
// 保証の範囲 (データ取得層は基準時刻を受け取らない) は backend の
// strategy_task.as_of の定義と揃える。
export const withAsOf = (promptText: string, asOf: Date | undefined): string =>
  asOf === undefined
    ? promptText
    : [
        `基準時刻 (as_of): ${asOf.toISOString()}`,
        'これは実行の論理的な基準時刻であり、参照するデータがすべてこの時刻のものであることは保証されない。',
        promptText,
      ].join('\n\n')
