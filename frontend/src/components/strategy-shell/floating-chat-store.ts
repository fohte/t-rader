import { useSyncExternalStore } from 'react'

// フローティングチャットの開閉と seed プロンプトを保持する最小ストア。
// 「アナリストに聞く」ボタンが seed を投げ込み、チャット側がそれを購読する。
interface ChatState {
  open: boolean
  seed: string | null
}

let state: ChatState = { open: false, seed: null }
const listeners = new Set<() => void>()

function emit() {
  for (const l of listeners) l()
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

function getSnapshot(): ChatState {
  return state
}

export function useFloatingChat(): ChatState {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot)
}

export function openFloatingChat(seed?: string | null): void {
  state = { open: true, seed: seed ?? null }
  emit()
}

export function closeFloatingChat(): void {
  state = { ...state, open: false }
  emit()
}

export function consumeFloatingChatSeed(): string | null {
  const s = state.seed
  if (s == null) return null
  state = { ...state, seed: null }
  emit()
  return s
}
