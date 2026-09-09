export const RATIO_PERCENT_ERROR = '0 より大きく 100 以下の値を入力してください'

export type ParsedRatioPercent =
  { ratio: number | null; error: null } | { ratio: null; error: string }

/**
 * パーセント表記の文字列 (0, 100] を比率 (0, 1] に変換する。空文字列は null を返す。
 */
export function parseRatioPercent(input: string): ParsedRatioPercent {
  const trimmed = input.trim()
  if (trimmed === '') return { ratio: null, error: null }
  const percent = Number(trimmed)
  if (!Number.isFinite(percent) || percent <= 0 || percent > 100) {
    return { ratio: null, error: RATIO_PERCENT_ERROR }
  }
  return { ratio: percent / 100, error: null }
}

/** *100 の浮動小数点誤差 (例: 0.07 -> 7.000000000000001) を丸めて表示用文字列にする */
export function formatRatioPercent(ratio: number | null | undefined): string {
  if (ratio == null) return ''
  return String(Number((ratio * 100).toFixed(6)))
}
