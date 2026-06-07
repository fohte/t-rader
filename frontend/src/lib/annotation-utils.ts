import type { components } from '@/lib/api/schema.gen'

type Annotation = components['schemas']['Annotation']

export type NumberedAnnotation = Annotation & {
  /** ピン番号 (A1, A2, ...) */
  label: string
}

/**
 * 指定 symbol のアノテーションを timestamp 昇順で並べピン番号 (A1, A2, ...) を割り当てる。
 *
 * チャートのマーカー描画とアノテーション一覧で番号付けが食い違わないよう、
 * 採番ロジックは必ずこの関数経由にすること。
 */
export function buildNumberedAnnotations(
  annotations: Annotation[],
  symbol: string | null | undefined,
): NumberedAnnotation[] {
  if (symbol == null) return []
  return annotations
    .filter((a) => a.target_symbol === symbol)
    .sort((a, b) => a.timestamp.localeCompare(b.timestamp))
    .map((a, i) => ({ ...a, label: `A${String(i + 1)}` }))
}
