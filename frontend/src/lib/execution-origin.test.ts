import { describe, expect, it } from 'vitest'

import { toExecutionStepId } from '#lib/execution-origin'

function readStepIds() {
  return [
    toExecutionStepId('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'),
    toExecutionStepId('not-a-uuid'),
    toExecutionStepId(null),
  ]
}

describe('toExecutionStepId', () => {
  it('accepts UUIDs and ignores values that are not UUIDs', () => {
    expect(readStepIds()).toEqual([
      'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      undefined,
      undefined,
    ])
  })
})
