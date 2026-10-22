import type { Result } from 'neverthrow'
import { err, ok } from 'neverthrow'

import { isPlainObject } from '#strategy-agent/agent-graph/json'
import type { PayloadReference } from '#strategy-agent/agent-graph/run-agent-graph/operation-runner'
import type { ExecutionToolRequest } from '#strategy-agent/execution-tool-client'
import type { McpToolsClient } from '#strategy-agent/strategy-agent'

export interface SealedPayloadItem {
  readonly value: unknown
  readonly itemKey: {
    readonly payload_id: string
    readonly pointer: string
    readonly index: number
  }
}

export interface SealedPayloadPage {
  readonly data: readonly unknown[]
  readonly cursor: number
  readonly nextCursor?: number
  readonly totalCount: number
  readonly digest: string
}

const callExecutionTool = (
  client: McpToolsClient,
  request: Extract<
    ExecutionToolRequest,
    { readonly name: 'read_execution_payload' }
  >,
): Promise<Result<unknown, Error>> =>
  client.callExecutionTool === undefined
    ? Promise.resolve(
        err(new Error('execution MCP tool dispatcher is not configured')),
      )
    : Promise.resolve()
        .then(() => client.callExecutionTool?.(request))
        .then(
          (result) =>
            result ??
            err(new Error('execution MCP tool dispatcher is not configured')),
        )
        .catch((error: unknown) =>
          err(error instanceof Error ? error : new Error(String(error))),
        )

const parseReadPayloadPage = (
  value: unknown,
  payloadId: string,
  pointer: string,
  requestedCursor: number,
  pageSize: number,
): Result<SealedPayloadPage, Error> => {
  if (!isPlainObject(value) || !Array.isArray(value['data'])) {
    return err(new Error('read_execution_payload returned a non-array page'))
  }
  const {
    payload_id: responsePayloadId,
    pointer: responsePointer,
    cursor,
    next_cursor: nextCursor,
    total_count: totalCount,
    digest,
  } = value
  if (
    responsePayloadId !== payloadId ||
    responsePointer !== pointer ||
    typeof cursor !== 'number' ||
    cursor !== requestedCursor ||
    typeof totalCount !== 'number' ||
    !Number.isSafeInteger(totalCount) ||
    totalCount < 0 ||
    typeof digest !== 'string' ||
    value['data'].length > pageSize ||
    (nextCursor !== null &&
      nextCursor !== undefined &&
      (typeof nextCursor !== 'number' ||
        !Number.isSafeInteger(nextCursor) ||
        nextCursor <= requestedCursor))
  ) {
    return err(
      new Error('read_execution_payload returned invalid page metadata'),
    )
  }
  if (
    Array.isArray(value['data']) &&
    nextCursor !== null &&
    nextCursor !== undefined &&
    value['data'].length === 0
  ) {
    return err(
      new Error('read_execution_payload returned an empty non-final page'),
    )
  }
  return ok({
    data: value['data'],
    cursor,
    ...(typeof nextCursor === 'number' ? { nextCursor } : {}),
    totalCount,
    digest,
  })
}

export const readSealedPayloadPages = async (
  client: McpToolsClient,
  payloadRef: PayloadReference,
  pointer: string,
  pageSize: number,
  onPage: (
    items: readonly SealedPayloadItem[],
    totalCount: number,
  ) => Promise<Result<void, unknown>>,
): Promise<Result<number, unknown>> => {
  if (client.callExecutionTool === undefined) {
    return err(new Error('execution MCP tool dispatcher is not configured'))
  }
  let cursor = 0
  let expectedTotalCount: number | undefined
  let expectedDigest = payloadRef.digest
  for (;;) {
    const page = await readSealedPayloadPage(
      client,
      payloadRef,
      pointer,
      cursor,
      pageSize,
    )
    if (page.isErr()) return err(page.error)
    if (
      expectedTotalCount !== undefined &&
      page.value.totalCount !== expectedTotalCount
    ) {
      return err(new Error('sealed payload item count changed between pages'))
    }
    expectedTotalCount = page.value.totalCount
    if (expectedDigest !== undefined && page.value.digest !== expectedDigest) {
      return err(new Error('sealed payload digest changed between pages'))
    }
    expectedDigest ??= page.value.digest
    const pageItems = page.value.data.map((value, offset) => ({
      value,
      itemKey: {
        payload_id: payloadRef.id,
        pointer,
        index: cursor + offset,
      },
    }))
    const processed = await onPage(pageItems, page.value.totalCount)
    if (processed.isErr()) return err(processed.error)
    if (page.value.nextCursor === undefined) {
      if (cursor + page.value.data.length !== page.value.totalCount) {
        return err(
          new Error('sealed payload ended before its declared item count'),
        )
      }
      return ok(page.value.totalCount)
    }
    if (page.value.nextCursor !== cursor + page.value.data.length) {
      return err(
        new Error('read_execution_payload returned a discontinuous page'),
      )
    }
    cursor = page.value.nextCursor
  }
}

export const readSealedPayloadPage = async (
  client: McpToolsClient,
  payloadRef: PayloadReference,
  pointer: string,
  cursor: number,
  limit: number,
): Promise<Result<SealedPayloadPage, Error>> => {
  const read = await callExecutionTool(client, {
    name: 'read_execution_payload',
    arguments: {
      payload_id: payloadRef.id,
      pointer,
      cursor,
      limit,
    },
  })
  if (read.isErr()) return err(read.error)
  return parseReadPayloadPage(read.value, payloadRef.id, pointer, cursor, limit)
}

export const payloadPointerForField = (
  phaseResult: unknown,
  field: string,
):
  | { readonly payloadRef: PayloadReference; readonly pointer: string }
  | undefined => {
  if (!isPlainObject(phaseResult)) return undefined
  const ref = phaseResult['payloadRef']
  if (!isPlainObject(ref) || typeof ref['id'] !== 'string') return undefined
  const basePointer =
    typeof phaseResult['pointer'] === 'string' ? phaseResult['pointer'] : ''
  const escapedField = field.replaceAll('~', '~0').replaceAll('/', '~1')
  return {
    payloadRef: {
      id: ref['id'],
      ...(typeof ref['digest'] === 'string' ? { digest: ref['digest'] } : {}),
    },
    pointer: `${basePointer}/${escapedField}`,
  }
}
