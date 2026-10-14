import { createFileRoute, useLocation } from '@tanstack/react-router'
import { useEffect, useState } from 'react'

import { DetailBackLink } from '#components/detail-back-link'
import { HistoricalVersionNotice } from '#components/note-detail/historical-version-notice'
import { MarkdownBody } from '#components/note-detail/markdown-body'
import { createNoteBodyNavigationHandlers } from '#components/note-detail/note-body-navigation'
import { NoteHistoryPanel } from '#components/note-detail/note-history-panel'
import { NoteLinksPanelView } from '#components/note-detail/note-links-panel'
import { NoteVersionContentTabs } from '#components/note-detail/note-version-content-tabs'
import { NoteVersionDiffPanel } from '#components/note-detail/note-version-diff-panel'
import { NoteVersionFallback } from '#components/note-detail/note-version-fallback'
import { NoteVersionHeader } from '#components/note-detail/note-version-header'
import { NoteVersionReviewNavigationView } from '#components/note-detail/note-version-review-navigation-view'
import { NoteVersionReviewPanel } from '#components/note-detail/note-version-review-panel'
import { PredictionsPanel } from '#components/note-detail/predictions-panel'
import {
  type ReviewNavigationSnapshot,
  type ReviewQueueItem,
  reviewQueueItemKey,
  reviewQueueModel,
  type ReviewQueueNavigationState,
} from '#components/review/review-queue-model'
import { useReviewQueue } from '#components/review/use-review-queue'
import { openFloatingChat } from '#components/strategy-shell/floating-chat-store'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/notes/$noteId')({
  validateSearch: (
    search: Record<string, unknown>,
  ): { version?: number; version_id?: string } => {
    const version = search.version
    const validated: { version?: number; version_id?: string } = {}
    if (
      typeof version === 'number' &&
      Number.isInteger(version) &&
      version > 0
    ) {
      validated.version = version
    } else if (typeof version === 'string') {
      const parsed = Number(version)
      if (Number.isInteger(parsed) && parsed > 0) validated.version = parsed
    }
    if (typeof search.version_id === 'string') {
      validated.version_id = search.version_id
    }
    return validated
  },
  component: NoteDetailPage,
})

function NoteDetailPage() {
  const { noteId } = Route.useParams()
  const { version: requestedVersionNo, version_id: requestedVersionId } =
    Route.useSearch()
  const navigate = Route.useNavigate()
  const location = useLocation()
  const reviewQueue = useReviewQueue()
  const openedReviewQueue = (
    location.state as typeof location.state & {
      reviewQueue?: ReviewQueueNavigationState
    }
  ).reviewQueue
  const [fallbackNavigation, setFallbackNavigation] = useState<{
    noteId: string
    snapshot: ReviewNavigationSnapshot | null
  }>(() => ({ noteId, snapshot: null }))
  const [contentTab, setContentTab] = useState<{
    versionId: string
    tab: 'changes' | 'full'
  } | null>(null)
  const [reviewedVersion, setReviewedVersion] = useState<{
    versionId: string
    status: 'approved' | 'rejected'
  } | null>(null)
  const noteBodyNavigation = createNoteBodyNavigationHandlers(navigate)
  const {
    data: versions,
    isPending: areVersionsPending,
    isError: hasVersionsError,
  } = $api.useQuery('get', '/api/notes/{id}/versions', {
    params: { path: { id: noteId } },
  })
  const { data: note } = $api.useQuery('get', '/api/notes/{id}', {
    params: { path: { id: noteId } },
  })
  const {
    data: noteHistory,
    isPending: isNoteHistoryPending,
    isError: hasNoteHistoryError,
  } = $api.useQuery('get', '/api/history', {
    params: { query: { target_kind: 'note', target_id: noteId, limit: 50 } },
  })

  const orderedVersions = [...(versions ?? [])].sort(
    (left, right) => left.version_no - right.version_no,
  )
  const selectedVersion =
    (requestedVersionId == null
      ? undefined
      : orderedVersions.find((version) => version.id === requestedVersionId)) ??
    orderedVersions.find(
      (version) => version.version_no === requestedVersionNo,
    ) ??
    orderedVersions.find((version) => version.is_current) ??
    orderedVersions.at(-1)
  const activeFallbackSnapshot =
    fallbackNavigation.noteId === noteId ? fallbackNavigation.snapshot : null
  const navigationSnapshot =
    openedReviewQueue?.snapshot ??
    activeFallbackSnapshot ??
    (reviewQueue.data == null
      ? null
      : reviewQueueModel.createNavigationSnapshot(reviewQueue.data.items))
  const queueNavigationState = reviewQueue.isError
    ? ({ kind: 'error' } as const)
    : reviewQueue.data == null || navigationSnapshot == null
      ? ({ kind: 'loading' } as const)
      : reviewQueueModel.getNavigation(
          navigationSnapshot,
          openedReviewQueue?.currentItemKey ??
            (selectedVersion == null ? '' : `note:${selectedVersion.id}`),
          reviewQueue.data.items,
        )
  const showInlineReview =
    selectedVersion != null &&
    (selectedVersion.status === 'unread' ||
      reviewedVersion?.versionId === selectedVersion.id)
  const activeContentTab =
    selectedVersion != null && contentTab?.versionId === selectedVersion.id
      ? contentTab.tab
      : 'changes'

  useEffect(() => {
    if (
      openedReviewQueue != null ||
      activeFallbackSnapshot != null ||
      reviewQueue.data == null
    ) {
      return
    }

    setFallbackNavigation({
      noteId,
      snapshot: reviewQueueModel.createNavigationSnapshot(
        reviewQueue.data.items,
      ),
    })
  }, [activeFallbackSnapshot, noteId, openedReviewQueue, reviewQueue.data])

  const navigateToReviewItem = (item: ReviewQueueItem): void => {
    if (navigationSnapshot == null) return
    const state: { reviewQueue: ReviewQueueNavigationState } = {
      reviewQueue: {
        snapshot: navigationSnapshot,
        currentItemKey: reviewQueueItemKey(item),
      },
    }

    if (item.kind === 'note') {
      void navigate({
        to: '/notes/$noteId',
        params: { noteId: item.noteId },
        search: { version: item.versionNo, version_id: item.id },
        state: (previous) => ({ ...previous, ...state }),
      })
      return
    }

    void navigate({
      to: '/annotations/$annoId',
      params: { annoId: item.id },
      state: (previous) => ({ ...previous, ...state }),
    })
  }

  const onReturnToReview = (): void => {
    void navigate({ to: '/review' })
  }
  const {
    data: noteLinks,
    isPending: linksPending,
    isError: linksError,
  } = $api.useQuery(
    'get',
    '/api/notes/{id}/links',
    {
      params: {
        path: { id: noteId },
        query: { version_id: selectedVersion?.id },
      },
    },
    { enabled: selectedVersion != null },
  )

  if (areVersionsPending || hasVersionsError || selectedVersion == null) {
    const fallbackState = areVersionsPending
      ? 'loading'
      : hasVersionsError
        ? 'error'
        : 'missing'

    return (
      <div className="space-y-4 font-sans text-foreground">
        <DetailBackLink label="ノート一覧に戻る" />
        <NoteVersionFallback state={fallbackState} />
      </div>
    )
  }

  const previousVersion =
    orderedVersions
      .filter((version) => version.version_no < selectedVersion.version_no)
      .at(-1) ?? null
  const onSelectContentTab = (tab: 'changes' | 'full'): void => {
    setContentTab({ versionId: selectedVersion.id, tab })
  }

  return (
    <div
      className={`space-y-4 font-sans text-foreground ${showInlineReview ? 'pb-20 lg:pb-0' : ''}`}
    >
      <DetailBackLink label="ノート一覧に戻る" />
      <NoteVersionReviewNavigationView
        state={queueNavigationState}
        onNavigateToItem={navigateToReviewItem}
        onReturnToReview={onReturnToReview}
      />
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-(--grid-cols-note-detail)">
        <article className="space-y-4 border border-border bg-card px-5 py-5">
          <NoteVersionHeader
            version={selectedVersion}
            noteKind={note?.kind ?? null}
            onAsk={(title) => {
              openFloatingChat('「' + title + '」について補足して')
            }}
          />
          {previousVersion == null ? (
            <MarkdownBody
              source={selectedVersion.body_md}
              graphs={selectedVersion.graphs_json}
              noteLinks={noteLinks?.outgoing}
              resolvedPriceReferences={
                selectedVersion.resolved_price_references_json
              }
              onRef={noteBodyNavigation.onRef}
              onAnno={noteBodyNavigation.onAnno}
            />
          ) : (
            <>
              <NoteVersionContentTabs
                activeTab={activeContentTab}
                onChange={onSelectContentTab}
              />
              {activeContentTab === 'changes' ? (
                <NoteVersionDiffPanel
                  key={selectedVersion.id}
                  version={selectedVersion}
                  previousVersion={previousVersion}
                />
              ) : (
                <MarkdownBody
                  source={selectedVersion.body_md}
                  graphs={selectedVersion.graphs_json}
                  noteLinks={noteLinks?.outgoing}
                  resolvedPriceReferences={
                    selectedVersion.resolved_price_references_json
                  }
                  onRef={noteBodyNavigation.onRef}
                  onAnno={noteBodyNavigation.onAnno}
                />
              )}
            </>
          )}
          {showInlineReview && (
            <NoteVersionReviewPanel
              key={selectedVersion.id}
              version={selectedVersion}
              isDecisionInline
              recentlyReviewedStatus={
                reviewedVersion?.versionId === selectedVersion.id
                  ? reviewedVersion.status
                  : null
              }
              onReviewed={(status) => {
                setReviewedVersion({ versionId: selectedVersion.id, status })
              }}
            />
          )}
        </article>
        <aside className="space-y-4">
          <NoteHistoryPanel
            versions={orderedVersions}
            history={noteHistory ?? []}
            isHistoryPending={isNoteHistoryPending}
            isHistoryError={hasNoteHistoryError}
            selectedVersionNo={selectedVersion.version_no}
            onSelectVersion={(versionNo) => {
              void navigate({
                search: { version: versionNo, version_id: undefined },
                state: (previous) => {
                  const previousReviewQueue = (
                    previous as typeof previous & {
                      reviewQueue?: ReviewQueueNavigationState
                    }
                  ).reviewQueue
                  if (
                    previousReviewQueue != null ||
                    navigationSnapshot == null
                  ) {
                    return previous
                  }
                  return {
                    ...previous,
                    reviewQueue: {
                      snapshot: navigationSnapshot,
                      currentItemKey: `note:${selectedVersion.id}`,
                    },
                  }
                },
              })
            }}
          />
          {!selectedVersion.is_current && (
            <HistoricalVersionNotice
              noteId={noteId}
              versionNo={selectedVersion.version_no}
            />
          )}
          {!showInlineReview && (
            <NoteVersionReviewPanel
              key={selectedVersion.id}
              version={selectedVersion}
            />
          )}
          <PredictionsPanel noteId={noteId} />
          <NoteLinksPanelView
            outgoing={noteLinks?.outgoing ?? []}
            incoming={noteLinks?.incoming ?? []}
            isPending={linksPending}
            isError={linksError}
          />
        </aside>
      </div>
    </div>
  )
}
