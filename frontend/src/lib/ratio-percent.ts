// backend の (0, 1] 範囲チェック (max_position_ratio / max_sector_ratio) と対称な、
// パーセント表記 (0, 100] での入出力を担う変換ヘルパー
export const RATIO_PERCENT_ERROR = '0 より大きく 100 以下の値を入力してください'

export type ParsedRatioPercent =
  { ratio: number | null; error: null } | { ratio: null; error: string }

/** 空欄は上限解除 (null) を表す */
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
