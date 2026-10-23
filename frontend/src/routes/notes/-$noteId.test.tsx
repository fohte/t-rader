import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  createMemoryHistory,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import type { components } from '#lib/api/schema.gen'
import { resetFetch, stubFetch } from '#test/stub-fetch.test-helper'

type Note = components['schemas']['Note']
type NoteVersion = components['schemas']['NoteVersion']
type NoteKind = components['schemas']['NoteKind']
const noteId = 'fake-note'
const versionId = 'fake-note-version'
const noteKindKey = 'fabricated-kind-key'
const noteKindLabel = '架空の種別表示名'
const rawBody =
  '[[stock:DEMO-A]] 架空工業: [[stock:DEMO-B]] 架空商事: 内容の続き'
const displayBody = '[[stock:DEMO-A]]: [[stock:DEMO-B]]: 内容の続き'

const historicalVersion: NoteVersion = {
  id: 'fake-historical-version',
  note_id: noteId,
  version_no: 1,
  is_current: false,
  title: '架空の分析ノート',
  body_md: rawBody,
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  status: 'approved',
  created_by_kind: 'human',
  created_at: '2099-01-01T00:00:00Z',
}

const currentVersion: NoteVersion = {
  ...historicalVersion,
  id: versionId,
  version_no: 2,
  is_current: true,
  title: '架空の分析ノート',
  body_md: '[[stock:DEMO-A]] 架空工業: 現行版の内容',
  status: 'approved',
}

const pendingVersion: NoteVersion = {
  ...historicalVersion,
  id: versionId,
  is_current: false,
  body_md: '架空の本文です。',
  status: 'unread',
  created_by_kind: 'llm',
}

function noteForVersion(
  version: NoteVersion,
  body = displayBody,
  kind: string | null = noteKindKey,
): Note {
  return {
    id: noteId,
    version_id: version.id,
    version_no: version.version_no,
    is_current: version.is_current,
    title: version.title,
    body_md: version.body_md,
    display_body_md: body,
    frontmatter_json: {},
    resolved_price_references_json: {},
    graphs_json: [],
    tags: [],
    kind,
    status: version.status,
    created_by_kind: version.created_by_kind,
    created_at: version.created_at,
    updated_at: version.created_at,
    execution_id: null,
  }
}

const noteKind: NoteKind = {
  key: noteKindKey,
  display_name: noteKindLabel,
  requires_approval: true,
  sort_order: 0,
}

type NoteRequest = {
  method: string
  path: string
  versionId: string | null
}

function setupFetch(
  versions: NoteVersion[],
  fetchNote: (request: Request) => Promise<Response>,
  fetchNoteKinds: () => Promise<Response> = () =>
    Promise.resolve(Response.json([noteKind])),
) {
  const noteRequests: NoteRequest[] = []
  stubFetch((input, init) => {
    const request = input instanceof Request ? input : new Request(input, init)
    const url = new URL(request.url)

    if (url.pathname === `/api/notes/${noteId}/versions`) {
      return Promise.resolve(Response.json(versions))
    }
    if (url.pathname === `/api/notes/${noteId}`) {
      noteRequests.push({
        method: request.method,
        path: url.pathname,
        versionId: url.searchParams.get('version_id'),
      })
      return fetchNote(request)
    }
    if (url.pathname === '/api/refs/resolve') {
      const link = url.searchParams.get('link')
      const refs: Record<string, string> = {
        'stock:DEMO-A': '架空工業',
        'stock:DEMO-B': '架空商事',
        'indicator:demo-indicator': 'デモ指標',
      }
      const name = link == null ? undefined : refs[link]
      const separator = link?.indexOf(':') ?? -1
      return Promise.resolve(
        Response.json([
          {
            kind: separator < 0 ? 'stock' : link?.slice(0, separator),
            id: separator < 0 ? (link ?? '') : link?.slice(separator + 1),
            name: name ?? null,
          },
        ]),
      )
    }
    if (url.pathname === '/api/note-kinds') {
      return fetchNoteKinds()
    }
    if (url.pathname === `/api/notes/${noteId}/links`) {
      return Promise.resolve(Response.json({ incoming: [], outgoing: [] }))
    }
    if (url.pathname === '/api/history') {
      return Promise.resolve(Response.json([]))
    }
    if (
      url.pathname === '/api/strategies' ||
      url.pathname === '/api/agent-configs'
    ) {
      return Promise.resolve(Response.json([]))
    }
    if (
      url.pathname === `/api/notes/${noteId}/trades` ||
      url.pathname === `/api/notes/${noteId}/predictions` ||
      url.pathname === '/api/comments' ||
      url.pathname === '/api/note-versions/pending' ||
      url.pathname === '/api/annotations'
    ) {
      return Promise.resolve(Response.json([]))
    }
    return Promise.resolve(Response.json({}, { status: 404 }))
  })
  return { noteRequests }
}

async function renderNoteDetail(
  versions: NoteVersion[],
  fetchNote: (request: Request) => Promise<Response>,
  initialEntry = `/notes/${noteId}`,
  fetchNoteKinds?: () => Promise<Response>,
) {
  const requests = setupFetch(versions, fetchNote, fetchNoteKinds)
  const { routeTree } = await import('#routeTree.gen')
  const router = createRouter({
    routeTree,
    history: createMemoryHistory({ initialEntries: [initialEntry] }),
  })
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  render(<RouterProvider router={router} />, {
    wrapper: ({ children }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    ),
  })
  return { ...requests, router }
}

function displayedBodyText(): string | null {
  const body = screen.queryByRole('region', { name: 'ノート本文' })
  if (body == null) return null
  const paragraphs = Array.from(body.querySelectorAll('p')).map(
    (paragraph) => paragraph.textContent,
  )
  return paragraphs.length === 0 ? null : paragraphs.join('\n')
}

function selectedVersionIndicatorOutput(
  chip: HTMLElement,
  noteRequests: NoteRequest[],
) {
  return {
    body: displayedBodyText(),
    chip: chip.textContent,
    requests: noteRequests,
  }
}

function notePageOutput(noteRequests: NoteRequest[]) {
  return [
    screen.queryByRole('heading', { level: 1 })?.textContent ?? null,
    noteKindOutput(),
    displayedBodyText(),
    screen.queryByRole('alert')?.textContent ?? null,
    screen.queryByRole('heading', { name: 'バージョン差分' })?.textContent ??
      null,
    noteRequests,
  ]
}

function noteKindOutput(): string | null {
  return (
    screen.queryByText(noteKindLabel)?.textContent ??
    screen.queryByText('種別なし')?.textContent ??
    screen.queryByText('種別を読み込み中')?.textContent ??
    screen.queryByText('種別を取得できません')?.textContent ??
    screen.queryByText('未登録の種別')?.textContent ??
    null
  )
}

function getCatalogLoadingOutput() {
  return {
    kindLabel: noteKindOutput(),
    bodyAvailable: displayedBodyText() != null,
  }
}

function getCatalogErrorOutput() {
  return {
    kindLabel: noteKindOutput(),
    bodyAvailable: displayedBodyText() != null,
    pageError: screen.queryByRole('alert')?.textContent ?? null,
  }
}

function getUnsetOrUnregisteredKindOutput(kind: string | null) {
  return {
    label: noteKindOutput(),
    keyVisible:
      kind == null ? false : screen.queryByText(kind, { exact: true }) != null,
    kindLink: screen.queryByRole('link', { name: /のノートを表示/ }) != null,
  }
}

afterEach(() => {
  cleanup()
  resetFetch()
})

describe('NoteDetailPage', () => {
  it('shows the selected version’s saved indicator change as a percentage chip', async () => {
    const token = '[[change:indicator:demo-indicator@2030-01-02..2030-01-03]]'
    const version: NoteVersion = {
      ...historicalVersion,
      body_md: token,
      resolved_price_references_json: { [token]: { value: 12.5 } },
    }
    const note = noteForVersion(version, token)
    note.resolved_price_references_json = { [token]: { value: 12.5 } }
    const requests = await renderNoteDetail(
      [version, currentVersion],
      () => Promise.resolve(Response.json(note)),
      `/notes/${noteId}?version_id=${version.id}`,
    )

    const chip = await screen.findByRole(
      'button',
      { name: '+12.5% の根拠を開く' },
      { timeout: 30_000 },
    )

    expect(selectedVersionIndicatorOutput(chip, requests.noteRequests)).toEqual(
      {
        body: '+12.5%',
        chip: '+12.5%',
        requests: [
          {
            method: 'GET',
            path: `/api/notes/${noteId}`,
            versionId: version.id,
          },
        ],
      },
    )
  }, 30_000)

  it('shows the selected historical version with its corrected display body', async () => {
    const requests = await renderNoteDetail(
      [historicalVersion, currentVersion],
      () => Promise.resolve(Response.json(noteForVersion(historicalVersion))),
      `/notes/${noteId}?version_id=${historicalVersion.id}`,
    )

    await screen.findByRole(
      'button',
      { name: '銘柄架空工業DEMO-A' },
      { timeout: 30_000 },
    )

    expect(notePageOutput(requests.noteRequests)).toEqual([
      historicalVersion.title,
      noteKindLabel,
      '銘柄架空工業DEMO-A: 銘柄架空商事DEMO-B: 内容の続き',
      null,
      null,
      [
        {
          method: 'GET',
          path: `/api/notes/${noteId}`,
          versionId: historicalVersion.id,
        },
      ],
    ])
  }, 30_000)

  it('keeps raw body hidden until a matching display response arrives', async () => {
    let resolveNoteResponse: (response: Response) => void = () => {}
    const response = new Promise<Response>((resolve) => {
      resolveNoteResponse = resolve
    })
    const requests = await renderNoteDetail(
      [historicalVersion],
      () => response,
      `/notes/${noteId}?version_id=${historicalVersion.id}`,
    )

    await screen.findByRole('heading', { level: 1 }, { timeout: 30_000 })
    await waitFor(() => {
      expect(requests.noteRequests).toEqual([
        {
          method: 'GET',
          path: `/api/notes/${noteId}`,
          versionId: historicalVersion.id,
        },
      ])
    })
    expect(displayedBodyText()).toEqual(null)

    resolveNoteResponse(Response.json(noteForVersion(historicalVersion)))
    await screen.findByRole(
      'button',
      { name: '銘柄架空工業DEMO-A' },
      { timeout: 30_000 },
    )

    expect(displayedBodyText()).toEqual(
      '銘柄架空工業DEMO-A: 銘柄架空商事DEMO-B: 内容の続き',
    )
  }, 30_000)

  it('keeps raw body hidden when the detail response belongs to another version', async () => {
    const mismatchedNote = noteForVersion(historicalVersion)
    mismatchedNote.version_id = currentVersion.id
    const requests = await renderNoteDetail(
      [historicalVersion, currentVersion],
      () => Promise.resolve(Response.json(mismatchedNote)),
      `/notes/${noteId}?version_id=${historicalVersion.id}`,
    )

    await screen.findByRole('heading', { level: 1 })
    await waitFor(() => {
      expect(notePageOutput(requests.noteRequests)).toEqual([
        historicalVersion.title,
        noteKindLabel,
        null,
        null,
        null,
        [
          {
            method: 'GET',
            path: `/api/notes/${noteId}`,
            versionId: historicalVersion.id,
          },
        ],
      ])
    })
  }, 30_000)

  it('keeps raw body hidden when the detail request fails', async () => {
    const requests = await renderNoteDetail(
      [historicalVersion],
      () => Promise.resolve(Response.json({}, { status: 500 })),
      `/notes/${noteId}?version_id=${historicalVersion.id}`,
    )

    await screen.findByRole('alert')

    expect(notePageOutput(requests.noteRequests)).toEqual([
      historicalVersion.title,
      '種別を取得できません',
      null,
      'バージョンを読み込めませんでした。',
      null,
      [
        {
          method: 'GET',
          path: `/api/notes/${noteId}`,
          versionId: historicalVersion.id,
        },
      ],
    ])
  }, 30_000)

  it('keeps the note readable while the kind catalog is loading', async () => {
    let resolveCatalog: (response: Response) => void = () => {}
    const catalogResponse = new Promise<Response>((resolve) => {
      resolveCatalog = resolve
    })
    await renderNoteDetail(
      [historicalVersion],
      () => Promise.resolve(Response.json(noteForVersion(historicalVersion))),
      undefined,
      () => catalogResponse,
    )

    await screen.findByText('種別を読み込み中')
    await screen.findByRole('region', { name: 'ノート本文' })
    resolveCatalog(Response.json([noteKind]))
    await screen.findByText(noteKindLabel)

    expect(getCatalogLoadingOutput()).toEqual({
      kindLabel: noteKindLabel,
      bodyAvailable: true,
    })
  }, 30_000)

  it('keeps the note readable when the kind catalog request fails', async () => {
    await renderNoteDetail(
      [historicalVersion],
      () => Promise.resolve(Response.json(noteForVersion(historicalVersion))),
      undefined,
      () => Promise.resolve(Response.json({}, { status: 500 })),
    )

    await screen.findByText('種別を取得できません')
    await screen.findByRole('region', { name: 'ノート本文' })

    expect(getCatalogErrorOutput()).toEqual({
      kindLabel: '種別を取得できません',
      bodyAvailable: true,
      pageError: null,
    })
  }, 30_000)

  it.each([
    { kind: null, label: '種別なし' },
    { kind: 'unlisted-kind-key', label: '未登録の種別' },
  ])(
    'distinguishes notes with kind $kind',
    async ({ kind, label }) => {
      await renderNoteDetail([historicalVersion], () =>
        Promise.resolve(
          Response.json(noteForVersion(historicalVersion, displayBody, kind)),
        ),
      )

      await screen.findByText(label)

      expect(getUnsetOrUnregisteredKindOutput(kind)).toEqual({
        label,
        keyVisible: false,
        kindLink: false,
      })
    },
    30_000,
  )

  it('opens the filtered note list from a registered kind label', async () => {
    const result = await renderNoteDetail(
      [historicalVersion],
      () => Promise.resolve(Response.json(noteForVersion(historicalVersion))),
      `/notes/${noteId}?version=1&tag=sample-tag`,
    )

    const kindLink = await screen.findByRole('link', {
      name: `${noteKindLabel} のノートを表示`,
    })
    fireEvent.click(kindLink)
    await waitFor(() => {
      expect(result.router.state.location.pathname).toEqual('/notes')
    })

    expect(result.router.state.location.search).toEqual({ kind: noteKindKey })
  }, 30_000)

  it('shows the pending note kind while reading a version without a current marker', async () => {
    const pendingNote = noteForVersion(pendingVersion, pendingVersion.body_md)
    const requests = await renderNoteDetail([pendingVersion], () =>
      Promise.resolve(Response.json(pendingNote)),
    )

    await screen.findByText(noteKindLabel)
    await screen.findByRole('region', { name: 'ノート本文' })

    expect(notePageOutput(requests.noteRequests)).toEqual([
      pendingVersion.title,
      noteKindLabel,
      pendingVersion.body_md,
      null,
      null,
      [
        {
          method: 'GET',
          path: `/api/notes/${noteId}`,
          versionId,
        },
      ],
    ])
  }, 30_000)

  it('compares raw version bodies and displays the corrected body in the full tab', async () => {
    const currentDisplayBody = '[[stock:DEMO-A]]: 現行版の内容'
    const requests = await renderNoteDetail(
      [historicalVersion, currentVersion],
      () =>
        Promise.resolve(
          Response.json(noteForVersion(currentVersion, currentDisplayBody)),
        ),
      `/notes/${noteId}?version_id=${currentVersion.id}`,
    )

    await screen.findByRole('heading', { name: 'バージョン差分' })
    const diffSection = screen
      .getByRole('heading', { name: 'バージョン差分' })
      .closest('section')
    await waitFor(() => {
      expect(
        Array.from(diffSection?.querySelectorAll('pre') ?? []).map(
          (element) => element.textContent,
        ),
      ).toEqual([historicalVersion.body_md, currentVersion.body_md])
    })

    fireEvent.click(screen.getByRole('tab', { name: '全文' }))
    await screen.findByRole('button', { name: '銘柄架空工業DEMO-A' })

    expect(notePageOutput(requests.noteRequests)).toEqual([
      currentVersion.title,
      noteKindLabel,
      '銘柄架空工業DEMO-A: 現行版の内容',
      null,
      null,
      [
        {
          method: 'GET',
          path: `/api/notes/${noteId}`,
          versionId: currentVersion.id,
        },
      ],
    ])
  }, 30_000)
})
