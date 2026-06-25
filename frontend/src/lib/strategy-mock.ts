export type RefKind = 'stock' | 'indicator' | 'sector' | 'theme'

export interface Strategy {
  id: string
  name: string
  horizon: string
  desc: string
  updatedAt: string
  unread: number
}

export interface RefEntity {
  kind: RefKind
  name: string
  sub?: string
}

export const STRATEGIES_MOCK: Strategy[] = [
  {
    id: 'semi-swing',
    name: '半導体短期スイング',
    horizon: 'SWING · 数週間',
    desc: '半導体セクターの循環と USD/JPY 連動を使った数週間スパンのスイング。',
    updatedAt: '2 時間前',
    unread: 3,
  },
  {
    id: 'rate-cycle',
    name: '米利上げサイクル長期観察',
    horizon: 'LONG · 数ヶ月〜',
    desc: 'FRB の利上げ/利下げサイクルとリスク資産の関係を長期で観察。',
    updatedAt: '昨日',
    unread: 1,
  },
  {
    id: 'value-long',
    name: '高配当バリュー長期',
    horizon: 'LONG · 数年',
    desc: '内需・高配当・キャッシュリッチ銘柄を長期ホールド。',
    updatedAt: '3 日前',
    unread: 0,
  },
]

export const REF_KIND_JP: Record<RefKind, string> = {
  stock: '銘柄',
  indicator: '指標',
  sector: 'セクター',
  theme: 'テーマ',
}

const REF_LABELS: Record<string, RefEntity> = {
  'stock:3436': { kind: 'stock', name: 'SUMCO', sub: '3436' },
  'stock:8035': { kind: 'stock', name: '東京エレクトロン', sub: '8035' },
  'stock:7203': { kind: 'stock', name: 'トヨタ自動車', sub: '7203' },
  'stock:6920': { kind: 'stock', name: 'レーザーテック', sub: '6920' },
  'indicator:USDJPY': { kind: 'indicator', name: 'USD/JPY' },
  'indicator:US10Y': { kind: 'indicator', name: '米10年金利' },
  'indicator:VIX': { kind: 'indicator', name: 'VIX' },
  'indicator:SOX': { kind: 'indicator', name: 'SOX指数' },
  'indicator:DXY': { kind: 'indicator', name: 'ドル指数' },
  'sector:半導体': { kind: 'sector', name: '半導体' },
  'theme:米利上げサイクル': { kind: 'theme', name: '米利上げサイクル' },
  'theme:円安': { kind: 'theme', name: '円安' },
  'theme:高配当': { kind: 'theme', name: '高配当' },
}

function isRefKind(s: string): s is RefKind {
  return s in REF_KIND_JP
}

export function resolveRef(token: string): RefEntity {
  const known = REF_LABELS[token]
  if (known) return known
  const i = token.indexOf(':')
  if (i < 0) return { kind: 'stock', name: token }
  const prefix = token.slice(0, i)
  const kind: RefKind = isRefKind(prefix) ? prefix : 'stock'
  return { kind, name: token.slice(i + 1) }
}
