import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'

import { DetailBackLink } from '#components/detail-back-link'
import { HistoricalVersionNotice } from '#components/note-detail/historical-version-notice'
import { MarkdownBody } from '#components/note-detail/markdown-body'
import { createNoteBodyNavigationHandlers } from '#components/note-detail/note-body-navigation'
import { NoteHistoryPanel } from '#components/note-detail/note-history-panel'
import { NoteLinkedTradesPanel } from '#components/note-detail/note-linked-trades-panel'
import { NoteLinksPanelView } from '#components/note-detail/note-links-panel'
import {
  getScreenerIdFromFrontmatter,
  NoteScreenerLink,
} from '#components/note-detail/note-screener-link'
import {
  type NoteVersionContentTab,
  NoteVersionContentTabs,
  resolveNoteVersionContentTab,
} from '#components/note-detail/note-version-content-tabs'
import { NoteVersionDiffPanel } from '#components/note-detail/note-version-diff-panel'
import { NoteVersionFallback } from '#components/note-detail/note-version-fallback'
import { NoteVersionHeader } from '#components/note-detail/note-version-header'
import { NoteVersionReviewPanel } from '#components/note-detail/note-version-review-panel'
import type { NoteVersionReviewResult } from '#components/note-detail/note-version-review-view'
import { PredictionsPanel } from '#components/note-detail/predictions-panel'
import { ReviewNavigationBand } from '#components/review/review-navigation-band'
import { reviewQueueItemKey } from '#components/review/review-queue-model'
import { useReviewNavigation } from '#components/review/use-review-navigation'
import {
  openFloatingChat,
  useChatContext,
} from '#components/strategy-shell/floating-chat-store'
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
  const [contentTab, setContentTab] = useState<{
    versionId: string
    tab: NoteVersionContentTab
  } | null>(null)
  const [reviewedVersion, setReviewedVersion] = useState<{
    versionId: string
    status: NoteVersionReviewResult
  } | null>(null)
  const noteBodyNavigation = createNoteBodyNavigationHandlers(navigate)
  const {
    data: versions,
    isPending: areVersionsPending,
    isError: hasVersionsError,
  } = $api.useQuery('get', '/api/notes/{id}/versions', {
    params: { path: { id: noteId } },
  })
  const { data: note, isError: hasNoteError } = $api.useQuery(
    'get',
    '/api/notes/{id}',
    { params: { path: { id: noteId } } },
  )
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
  const previousVersion =
    selectedVersion == null
      ? null
      : (orderedVersions
          .filter((version) => version.version_no < selectedVersion.version_no)
          .at(-1) ?? null)
  const showInlineReview =
    selectedVersion != null &&
    (selectedVersion.status === 'unread' ||
      reviewedVersion?.versionId === selectedVersion.id)
  const selectedContentTab =
    selectedVersion != null && contentTab?.versionId === selectedVersion.id
      ? contentTab.tab
      : null
  const activeContentTab = resolveNoteVersionContentTab(selectedContentTab)
  const reviewNavigation = useReviewNavigation(
    selectedVersion == null
      ? ''
      : reviewQueueItemKey({ kind: 'note', id: selectedVersion.id }),
  )
  const {
    navigation,
    isPending: isReviewNavigationPending,
    isError: isReviewNavigationError,
  } = reviewNavigation
  useChatContext({ kind: 'note', id: noteId, label: selectedVersion?.title })
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

  const onSelectContentTab = (tab: NoteVersionContentTab): void => {
    setContentTab({ versionId: selectedVersion.id, tab })
  }
  const fullBody = (
    <MarkdownBody
      source={selectedVersion.body_md}
      graphs={selectedVersion.graphs_json}
      noteLinks={noteLinks?.outgoing}
      resolvedPriceReferences={selectedVersion.resolved_price_references_json}
      onRef={noteBodyNavigation.onRef}
      onAnno={noteBodyNavigation.onAnno}
    />
  )
  const screenId = getScreenerIdFromFrontmatter(
    selectedVersion.frontmatter_json,
  )

  return (
    <div
      className={`space-y-4 font-sans text-foreground ${showInlineReview ? 'pb-20 lg:pb-0' : ''}`}
    >
      <DetailBackLink label="ノート一覧に戻る" />
      {navigation != null ? (
        <ReviewNavigationBand {...navigation} />
      ) : isReviewNavigationError ? (
        <ReviewNavigationBand isError />
      ) : isReviewNavigationPending ? (
        <ReviewNavigationBand isPending />
      ) : null}
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-(--grid-cols-note-detail)">
        <article className="space-y-4 border border-border bg-card px-5 py-5">
          <NoteVersionHeader
            version={selectedVersion}
            noteKind={note?.kind ?? null}
            hasNoteError={hasNoteError}
            onAsk={(title) => {
              openFloatingChat('「' + title + '」について補足して')
            }}
          />
          {previousVersion == null ? (
            fullBody
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
                fullBody
              )}
            </>
          )}
          <NoteScreenerLink screenId={screenId} />
          {showInlineReview && (
            <NoteVersionReviewPanel
              key={selectedVersion.id}
              version={selectedVersion}
              pinDecisionActionsOnMobile
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
              const targetVersion = orderedVersions.find(
                (version) => version.version_no === versionNo,
              )
              void navigate({
                search: { version: versionNo, version_id: undefined },
                state: (previous) => {
                  const snapshot = reviewNavigation.navigation?.snapshot
                  if (snapshot == null || targetVersion == null) return previous
                  return {
                    ...previous,
                    reviewQueue: {
                      snapshot,
                      currentItemKey: reviewQueueItemKey({
                        kind: 'note',
                        id: targetVersion.id,
                      }),
                    },
                  }
                },
              })
            }}
          />
          <NoteLinkedTradesPanel noteId={noteId} />
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
          <NoteLinksPanelView
            outgoing={noteLinks?.outgoing ?? []}
            incoming={noteLinks?.incoming ?? []}
            isPending={linksPending}
            isError={linksError}
          />
          <PredictionsPanel noteId={noteId} />
        </aside>
      </div>
    </div>
  )
}
