import type { ReviewNavigationState } from '#components/review/review-queue-model'

declare module '@tanstack/history' {
  interface HistoryState {
    reviewQueue?: ReviewNavigationState
  }
}
