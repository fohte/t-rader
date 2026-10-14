import { useEffect, useSyncExternalStore } from 'react'

import type { components } from '#lib/api/schema.gen'

export type ChatContextKind = components['schemas']['StrategyTaskContextKind']

export interface ChatContext {
  kind: ChatContextKind
  id: string
  label?: string
}

export interface ChatContextRegistration extends ChatContext {
  strategyId?: string
}

// 画面と常時表示チャットが独立しているため、起動操作と画面文脈を共有する。
interface ChatState {
  open: boolean
  seed: string | null
  context: ChatContext | null
  contextStrategyId: string | null
  contextRegistrationId: number | null
}

let state: ChatState = {
  open: false,
  seed: null,
  context: null,
  contextStrategyId: null,
  contextRegistrationId: null,
}
let nextContextRegistrationId = 0
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

export function useChatContext(context: ChatContextRegistration | null): void {
  const kind = context?.kind
  const id = context?.id
  const label = context?.label
  const strategyId = context?.strategyId

  useEffect(() => {
    if (kind == null || id == null) return
    return registerChatContext({ kind, id, label, strategyId })
  }, [id, kind, label, strategyId])
}

function registerChatContext(context: ChatContextRegistration): () => void {
  const registrationId = ++nextContextRegistrationId
  state = {
    ...state,
    context: { kind: context.kind, id: context.id, label: context.label },
    contextStrategyId: context.strategyId ?? null,
    contextRegistrationId: registrationId,
  }
  emit()

  return () => {
    if (state.contextRegistrationId !== registrationId) return
    state = {
      ...state,
      context: null,
      contextStrategyId: null,
      contextRegistrationId: null,
    }
    emit()
  }
}

export function removeChatContext(): void {
  if (state.context == null) return
  state = { ...state, context: null }
  emit()
}

export function openFloatingChat(seed?: string | null): void {
  const next: ChatState = { ...state, open: true, seed: seed ?? null }
  if (state.open === next.open && state.seed === next.seed) return
  state = next
  emit()
}

export function closeFloatingChat(): void {
  if (!state.open) return
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
