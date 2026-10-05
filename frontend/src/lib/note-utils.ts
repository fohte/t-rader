const REF_KINDS_ALT = ['stock', 'indicator', 'group'].join('|')
const REF_RE = new RegExp(`\\[\\[(${REF_KINDS_ALT}):([^\\]]+)\\]\\]`, 'g')
export const REF_PREFIX_RE = new RegExp(`^(${REF_KINDS_ALT}):`)

// 本文からスニペットを抽出する。markdown 装飾はざっくり除去する。
export function buildSnippet(bodyMd: string, max = 140): string {
  const stripped = bodyMd
    .replace(REF_RE, (_, _kind, id: string) => id)
    .replace(/^#+\s*/gm, '')
    .replace(/[*_`>]/g, '')
    .replace(/\n+/g, ' ')
    .trim()
  return stripped.length > max ? `${stripped.slice(0, max)}…` : stripped
}

const SEC = 1
const MIN = 60
const HR = 60 * 60
const DAY = 24 * HR

export function formatRelative(iso: string, now = Date.now()): string {
  const t = new Date(iso).getTime()
  if (Number.isNaN(t)) return '—'
  const raw = Math.round((now - t) / 1000)
  if (raw < SEC) return 'たった今'
  const diff = raw
  if (diff < MIN) return `${String(diff)} 秒前`
  if (diff < HR) return `${String(Math.floor(diff / MIN))} 分前`
  if (diff < DAY) return `${String(Math.floor(diff / HR))} 時間前`
  if (diff < 2 * DAY) return '昨日'
  if (diff < 7 * DAY) return `${String(Math.floor(diff / DAY))} 日前`
  return iso.slice(0, 10)
}
