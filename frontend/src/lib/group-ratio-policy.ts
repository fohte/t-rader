import { parseRatioPercent } from '#lib/ratio-percent'

export interface GroupRatioDraft {
  axis: string
  ratioPercent: string
}

export interface GroupRatioLimit {
  axis: string
  ratio: number
}

export interface GroupRatioDraftErrors {
  axis?: string
  ratioPercent?: string
}

export interface ParsedGroupRatioDrafts {
  maxGroupRatios: GroupRatioLimit[]
  errors: Array<GroupRatioDraftErrors | null>
}

export function parseGroupRatioDrafts(
  rows: GroupRatioDraft[],
): ParsedGroupRatioDrafts {
  const seenAxes = new Set<string>()
  const errors: Array<GroupRatioDraftErrors | null> = []
  const maxGroupRatios: GroupRatioLimit[] = []

  for (const row of rows) {
    if (row.axis === '') {
      errors.push({ axis: '分類軸を選択してください' })
      continue
    }
    if (seenAxes.has(row.axis)) {
      errors.push({ axis: '分類軸は重複して設定できません' })
      continue
    }
    seenAxes.add(row.axis)

    const parsed = parseRatioPercent(row.ratioPercent)
    if (parsed.error != null) {
      errors.push({ ratioPercent: parsed.error })
      continue
    }
    if (parsed.ratio == null) {
      errors.push({ ratioPercent: '上限比率を入力してください' })
      continue
    }
    maxGroupRatios.push({ axis: row.axis, ratio: parsed.ratio })
    errors.push(null)
  }

  return { maxGroupRatios, errors }
}
