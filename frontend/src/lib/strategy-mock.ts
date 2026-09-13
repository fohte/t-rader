export interface Strategy {
  id: string
  name: string
  horizon: string
  desc: string
  updatedAt: string
  unread: number
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
