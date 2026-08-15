import { type RefObject, useEffect, useRef, useState } from 'react'

import { MarkdownBody } from '#components/note-detail/markdown-body'
import { openFloatingChat } from '#components/strategy-shell/floating-chat-store'
import type { components } from '#lib/api/schema.gen'

interface NoteDocumentProps {
  source: string
  graphs?: components['schemas']['GraphDef'][]
  onQuoteSelection: (text: string) => void
  bodyRef: RefObject<HTMLDivElement | null>
}

export function NoteDocument({
  source,
  graphs,
  onQuoteSelection,
  bodyRef,
}: NoteDocumentProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const [sel, setSel] = useState<{
    text: string
    x: number
    y: number
  } | null>(null)

  useEffect(() => {
    const el = bodyRef.current
    if (!el) return
    let rafId = 0
    const onUp = (): void => {
      // 直前に mouseup したばかりだと selection が確定していないため次フレームで読む。
      rafId = requestAnimationFrame(() => {
        const s = window.getSelection()
        if (!s || s.isCollapsed || s.rangeCount === 0) {
          setSel(null)
          return
        }
        const txt = s.toString().trim()
        if (txt === '' || !el.contains(s.anchorNode)) {
          setSel(null)
          return
        }
        const r = s.getRangeAt(0).getBoundingClientRect()
        const host = hostRef.current?.getBoundingClientRect()
        if (!host) return
        // 中央寄せだが host 内に収まるよう左右を clamp する。
        const half = 120
        const cx = r.left + r.width / 2 - host.left
        const x = Math.min(Math.max(half, cx), host.width - half)
        setSel({ text: txt, x, y: r.top - host.top })
      })
    }
    el.addEventListener('mouseup', onUp)
    return () => {
      el.removeEventListener('mouseup', onUp)
      if (rafId !== 0) cancelAnimationFrame(rafId)
    }
  }, [bodyRef])

  useEffect(() => {
    const onDown = (e: MouseEvent): void => {
      const target = e.target
      if (!(target instanceof Element)) return
      if (!target.closest('[data-sel-toolbar]')) setSel(null)
    }
    document.addEventListener('mousedown', onDown)
    return () => {
      document.removeEventListener('mousedown', onDown)
    }
  }, [])

  return (
    <div ref={hostRef} className="relative">
      {sel && (
        <div
          data-sel-toolbar
          style={{
            left: `${String(sel.x)}px`,
            top: `${String(sel.y - 8)}px`,
          }}
          className="absolute z-30 -translate-x-1/2 -translate-y-full border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-secondary)] p-1 font-mono text-[12px] shadow-lg"
        >
          <button
            type="button"
            onMouseDown={(e) => {
              e.preventDefault()
            }}
            onClick={() => {
              onQuoteSelection(sel.text)
              setSel(null)
            }}
            className="inline-flex items-center gap-1 px-2 py-1 text-[color:var(--color-text-primary)] hover:bg-[color:var(--panel-inset)]"
          >
            <span className="text-[color:var(--color-accent-strategy)]">+</span>
            コメント
          </button>
          <button
            type="button"
            onMouseDown={(e) => {
              e.preventDefault()
            }}
            onClick={() => {
              openFloatingChat(
                `選択箇所について調べて: 「${sel.text.slice(0, 200)}」`,
              )
              setSel(null)
            }}
            className="inline-flex items-center gap-1 px-2 py-1 text-[color:var(--color-text-primary)] hover:bg-[color:var(--panel-inset)]"
          >
            <span className="text-[color:var(--color-accent-strategy)]">
              &gt;_
            </span>
            アナリストに聞く
          </button>
        </div>
      )}
      <div ref={bodyRef}>
        <MarkdownBody source={source} graphs={graphs} />
      </div>
    </div>
  )
}
