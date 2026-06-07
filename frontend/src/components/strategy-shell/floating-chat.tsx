import { X } from 'lucide-react'
import { useEffect, useState } from 'react'

import {
  closeFloatingChat,
  consumeFloatingChatSeed,
  openFloatingChat,
  useFloatingChat,
} from '@/components/strategy-shell/floating-chat-store'

export function FloatingChat() {
  const { open, seed: storeSeed } = useFloatingChat()
  const [seed, setSeed] = useState<string | null>(null)
  const [input, setInput] = useState('')

  // open 中に新しい seed が投げ込まれたら input を差し替える。
  useEffect(() => {
    if (!open || storeSeed == null) return
    const s = consumeFloatingChatSeed()
    setSeed(s)
    setInput(s ?? '')
  }, [open, storeSeed])

  useEffect(() => {
    if (!open) return
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') closeFloatingChat()
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => {
      window.removeEventListener('keydown', handleKeyDown)
    }
  }, [open])

  if (!open) {
    return (
      <button
        type="button"
        onClick={() => {
          openFloatingChat()
        }}
        title="アナリストを呼ぶ (on-demand)"
        aria-label="アナリストを呼ぶ"
        className="fixed bottom-5 right-5 z-[60] grid h-12 w-12 cursor-pointer place-items-center border border-[color:var(--color-accent-strategy)] bg-[color:var(--color-bg-secondary)] font-mono text-[22px] font-bold text-[color:var(--color-accent-strategy)] hover:bg-[color:var(--color-accent-strategy)] hover:text-white"
      >
        &gt;_
      </button>
    )
  }

  return (
    <div
      role="dialog"
      aria-label="on-demand session"
      className="fixed bottom-5 right-5 z-[60] flex h-[580px] max-h-[calc(100vh-100px)] w-[420px] max-w-[calc(100vw-28px)] flex-col border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-secondary)]"
    >
      <div className="flex items-center gap-2.5 border-b border-[color:var(--color-border-strategy)] px-3.5 py-2.5">
        <span className="flex items-baseline gap-1.5 font-mono text-[13px] font-bold">
          <span className="text-[color:var(--color-accent-strategy)]">
            &gt;_
          </span>
          <span>on-demand session</span>
        </span>
        <button
          type="button"
          onClick={() => {
            closeFloatingChat()
          }}
          aria-label="閉じる"
          className="ml-auto cursor-pointer text-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-text-primary)]"
        >
          <X className="size-4" />
        </button>
      </div>
      <div className="flex-1 overflow-y-auto p-3.5 text-[13px] text-[color:var(--color-text-secondary)]">
        {seed != null ? (
          <div className="border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-primary)] p-3">
            <div className="mb-2 font-mono text-[10px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
              seed
            </div>
            <p className="leading-relaxed text-[color:var(--color-text-primary)]">
              {seed}
            </p>
          </div>
        ) : (
          <p className="leading-relaxed">未接続。</p>
        )}
      </div>
      <div className="flex items-center gap-2 border-t border-[color:var(--color-border-strategy)] px-3.5 py-3">
        <span className="font-mono font-bold text-[color:var(--color-accent-strategy)]">
          &gt;
        </span>
        <input
          disabled
          aria-label="メッセージ入力"
          value={input}
          onChange={(e) => {
            setInput(e.target.value)
          }}
          placeholder="未接続"
          className="flex-1 border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-primary)] px-2.5 py-2 font-mono text-[13px] text-[color:var(--color-text-primary)] outline-none"
        />
      </div>
    </div>
  )
}
