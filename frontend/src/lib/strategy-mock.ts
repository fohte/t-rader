export type RefKind = 'stock' | 'indicator' | 'group'

export const REF_KIND_JP: Record<RefKind, string> = {
  stock: '銘柄',
  indicator: '指標',
  group: 'グループ',
}
