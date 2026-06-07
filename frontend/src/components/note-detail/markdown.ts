import { REF_PREFIX_RE } from '@/lib/note-utils'

export type InlineToken =
  | { kind: 'text'; value: string }
  | { kind: 'bold'; value: string }
  | { kind: 'italic'; value: string }
  | { kind: 'code'; value: string }
  | { kind: 'ref'; token: string }
  | { kind: 'anno'; id: string }

export type Block =
  | { kind: 'h1'; inline: InlineToken[] }
  | { kind: 'h2'; inline: InlineToken[] }
  | { kind: 'h3'; inline: InlineToken[] }
  | { kind: 'p'; inline: InlineToken[] }
  | { kind: 'ul'; items: InlineToken[][] }
  | { kind: 'ol'; items: InlineToken[][] }
  | { kind: 'quote'; inline: InlineToken[] }
  | { kind: 'code'; value: string; lang: string | null }

const ANNO_RE = /^anno:([A-Za-z][\w-]*)$/

// `[[kind:id]]` を anno/ref に振り分け、それ以外を **bold** / *italic* / `code` に分解する。
export function parseInline(text: string): InlineToken[] {
  const tokens: InlineToken[] = []
  const linkRe = /\[\[([^\]]+)\]\]/g
  let last = 0
  for (const m of text.matchAll(linkRe)) {
    const idx = m.index
    if (idx > last) {
      pushPlain(tokens, text.slice(last, idx))
    }
    const inner = m[1] ?? ''
    const anno = ANNO_RE.exec(inner)
    if (anno) {
      tokens.push({ kind: 'anno', id: anno[1] ?? '' })
    } else if (REF_PREFIX_RE.test(inner)) {
      tokens.push({ kind: 'ref', token: inner })
    } else {
      pushPlain(tokens, m[0])
    }
    last = idx + m[0].length
  }
  if (last < text.length) pushPlain(tokens, text.slice(last))
  return tokens
}

function pushPlain(out: InlineToken[], text: string): void {
  const re = /(\*\*[^*]+\*\*|\*[^*\n]+\*|`[^`\n]+`)/g
  let last = 0
  for (const m of text.matchAll(re)) {
    const idx = m.index
    if (idx > last) out.push({ kind: 'text', value: text.slice(last, idx) })
    const raw = m[1] ?? ''
    if (raw.startsWith('**')) {
      out.push({ kind: 'bold', value: raw.slice(2, -2) })
    } else if (raw.startsWith('*')) {
      out.push({ kind: 'italic', value: raw.slice(1, -1) })
    } else {
      out.push({ kind: 'code', value: raw.slice(1, -1) })
    }
    last = idx + raw.length
  }
  if (last < text.length) out.push({ kind: 'text', value: text.slice(last) })
}

export function parseMarkdown(src: string): Block[] {
  const lines = src.replace(/\r\n?/g, '\n').split('\n')
  const blocks: Block[] = []
  let i = 0

  const flushParagraph = (buf: string[]): void => {
    if (buf.length === 0) return
    blocks.push({ kind: 'p', inline: parseInline(buf.join(' ')) })
    buf.length = 0
  }

  const paraBuf: string[] = []
  while (i < lines.length) {
    const line = lines[i] ?? ''
    const trimmed = line.trim()

    if (trimmed === '') {
      flushParagraph(paraBuf)
      i += 1
      continue
    }

    // code fence
    if (trimmed.startsWith('```')) {
      flushParagraph(paraBuf)
      const lang = trimmed.slice(3).trim() || null
      const codeLines: string[] = []
      i += 1
      while (i < lines.length && (lines[i] ?? '').trim() !== '```') {
        codeLines.push(lines[i] ?? '')
        i += 1
      }
      i += 1
      blocks.push({ kind: 'code', value: codeLines.join('\n'), lang })
      continue
    }

    const heading = /^(#{1,3})\s+(.*)$/.exec(trimmed)
    if (heading) {
      flushParagraph(paraBuf)
      const level = heading[1]?.length ?? 1
      const text = heading[2] ?? ''
      const kind = level === 1 ? 'h1' : level === 2 ? 'h2' : 'h3'
      blocks.push({ kind, inline: parseInline(text) })
      i += 1
      continue
    }

    if (/^[-*]\s+/.test(trimmed)) {
      flushParagraph(paraBuf)
      const items: InlineToken[][] = []
      while (i < lines.length) {
        const m = /^[-*]\s+(.*)$/.exec((lines[i] ?? '').trim())
        if (!m) break
        items.push(parseInline(m[1] ?? ''))
        i += 1
      }
      blocks.push({ kind: 'ul', items })
      continue
    }

    if (/^\d+\.\s+/.test(trimmed)) {
      flushParagraph(paraBuf)
      const items: InlineToken[][] = []
      while (i < lines.length) {
        const m = /^\d+\.\s+(.*)$/.exec((lines[i] ?? '').trim())
        if (!m) break
        items.push(parseInline(m[1] ?? ''))
        i += 1
      }
      blocks.push({ kind: 'ol', items })
      continue
    }

    if (/^>\s?/.test(trimmed)) {
      flushParagraph(paraBuf)
      const buf: string[] = []
      while (i < lines.length) {
        const m = /^>\s?(.*)$/.exec((lines[i] ?? '').trim())
        if (!m) break
        buf.push(m[1] ?? '')
        i += 1
      }
      blocks.push({ kind: 'quote', inline: parseInline(buf.join(' ')) })
      continue
    }

    paraBuf.push(trimmed)
    i += 1
  }
  flushParagraph(paraBuf)
  return blocks
}
