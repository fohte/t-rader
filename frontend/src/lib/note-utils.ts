const REF_RE = /\[\[(stock|indicator|sector|theme):([^\]]+)\]\]/g

interface RefSource {
  frontmatter_json: unknown
  body_md: string
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null
}

// frontmatter_json に refs があれば優先し、なければ本文から `[[kind:id]]` を抽出する。
export function extractRefs(note: RefSource): string[] {
  const fm = isRecord(note.frontmatter_json) ? note.frontmatter_json : null
  const raw = fm?.['refs']
  if (Array.isArray(raw)) {
    return raw.filter((v): v is string => typeof v === 'string')
  }
  const found: string[] = []
  const seen = new Set<string>()
  for (const m of note.body_md.matchAll(REF_RE)) {
    const kind = m[1] ?? ''
    const id = m[2] ?? ''
    const token = `${kind}:${id}`
    if (!seen.has(token)) {
      seen.add(token)
      found.push(token)
    }
  }
  return found
}

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

export function isNewerThan(iso: string, since: number | null): boolean {
  if (since == null) return true
  const t = new Date(iso).getTime()
  return !Number.isNaN(t) && t > since
}
