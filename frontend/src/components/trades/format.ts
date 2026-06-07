/** 取引履歴ページ専用の数値表示ユーティリティ。 */

const intFmt = new Intl.NumberFormat('en-US')

/** 円表示。signed=true で符号付き (+¥1,234 / −¥1,234)。 */
export function formatYen(value: number, signed = false): string {
  const rounded = Math.round(value)
  const abs = intFmt.format(Math.abs(rounded))
  if (signed) {
    if (rounded > 0) return `+¥${abs}`
    if (rounded < 0) return `−¥${abs}`
    return `¥${abs}`
  }
  return rounded < 0 ? `−¥${abs}` : `¥${abs}`
}

/** プラス/マイナス/ゼロを色に対応させる Tailwind クラス。 */
export function pnlColorClass(value: number): string {
  if (value > 0) return 'text-[color:var(--color-up)]'
  if (value < 0) return 'text-[color:var(--color-down)]'
  return 'text-[color:var(--color-text-secondary)]'
}

export const SOURCE_LABEL: Record<string, string> = {
  manual: '手入力',
  csv: 'CSV取込',
  api: 'API',
}
