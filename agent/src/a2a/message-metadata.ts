import type { Message } from '@a2a-js/sdk'

export const extractStrategyId = (message: Message): string | undefined => {
  const raw = message.metadata?.['strategy_id']
  return typeof raw === 'string' ? raw : undefined
}

export const extractPurpose = (message: Message): string | undefined => {
  const raw = message.metadata?.['purpose']
  return typeof raw === 'string' ? raw : undefined
}

export const extractResumeSteps = (message: Message): unknown[] | undefined => {
  const raw = message.metadata?.['resume_steps']
  return Array.isArray(raw) ? raw : undefined
}

const extractDate = (message: Message, key: string): Date | undefined => {
  const raw = message.metadata?.[key]
  if (typeof raw !== 'string') return undefined
  const parsed = new Date(raw)
  return Number.isNaN(parsed.getTime()) ? undefined : parsed
}

export const extractDeadlineAt = (message: Message): Date | undefined =>
  extractDate(message, 'deadline_at')

export const extractAsOf = (message: Message): Date | undefined =>
  extractDate(message, 'as_of')
