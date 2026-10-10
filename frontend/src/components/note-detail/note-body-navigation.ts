import type { ResolvedRef } from '#hooks/use-resolve-ref'

type NoteBodyNavigationTarget =
  | {
      to: '/charts/$instrumentId'
      params: { instrumentId: string }
    }
  | {
      to: '/annotations/$annoId'
      params: { annoId: string }
    }

type Navigate = (target: NoteBodyNavigationTarget) => unknown

export function createNoteBodyNavigationHandlers(navigate: Navigate) {
  return {
    onRef: (_token: string, resolved: ResolvedRef) => {
      void navigate({
        to: '/charts/$instrumentId',
        params: { instrumentId: resolved.id },
      })
    },
    onAnno: (annoId: string) => {
      void navigate({
        to: '/annotations/$annoId',
        params: { annoId },
      })
    },
  }
}
