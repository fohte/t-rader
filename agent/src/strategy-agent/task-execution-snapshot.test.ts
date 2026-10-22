import { err, ok, okAsync } from 'neverthrow'
import { describe, expect, it, vi } from 'vitest'

import type { SubagentDefinition } from '#strategy-agent/subagent-definition-client'
import {
  createSnapshotSubagentDefinitionFetcher,
  createTaskExecutionSnapshotFetcher,
} from '#strategy-agent/task-execution-snapshot'

const subagentResponse = {
  id: 'synthetic-definition-id',
  key: 'helper-synthetic',
  description: 'A synthetic helper',
  model: 'example-model-helper',
  reasoning_effort: null,
  prompt: 'Handle a synthetic task',
  tools: ['read_synthetic_data'],
  timeout_minutes: 5,
  max_model_calls: 3,
  tool_limits: { read_synthetic_data: 2 },
  output: { type: 'object' },
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}

const subagentDefinition: SubagentDefinition = {
  key: 'helper-synthetic',
  description: 'A synthetic helper',
  model: 'example-model-helper',
  prompt: 'Handle a synthetic task',
  tools: ['read_synthetic_data'],
  timeoutMinutes: 5,
  maxModelCalls: 3,
  toolLimits: { read_synthetic_data: 2 },
  output: { type: 'object' },
}

const fetchSnapshotResponse = (response: unknown) => {
  const fetchSnapshot = createTaskExecutionSnapshotFetcher(() => ({
    callExecutionTool: () => Promise.resolve(ok(response)),
    close: () => Promise.resolve(),
  }))
  return fetchSnapshot('strategy-synthetic', 'task-synthetic')
}

describe('createTaskExecutionSnapshotFetcher', () => {
  it('normalizes the captured agent config and child definitions', async () => {
    expect(
      await fetchSnapshotResponse({
        digest: 'synthetic-digest',
        definition: {
          agent_config: {
            agents_md: 'Synthetic agent rules',
            skills: { 'skill-synthetic': 'Synthetic skill' },
            agent_graph: 'phases: []',
          },
          subagent_definitions: [subagentResponse],
        },
      }),
    ).toEqual(
      ok({
        agentConfig: {
          agentsMd: 'Synthetic agent rules',
          skills: { 'skill-synthetic': 'Synthetic skill' },
          agentGraph: 'phases: []',
        },
        subagentDefinitions: new Map([
          ['helper-synthetic', subagentDefinition],
        ]),
      }),
    )
  })

  it('returns no snapshot for a task created before snapshot support', async () => {
    expect(
      await fetchSnapshotResponse({ definition: null, digest: null }),
    ).toEqual(ok(undefined))
  })

  it('rejects a snapshot with duplicate child definition keys', async () => {
    expect(
      await fetchSnapshotResponse({
        digest: 'synthetic-digest',
        definition: {
          agent_config: {
            agents_md: '',
            skills: {},
            agent_graph: 'phases: []',
          },
          subagent_definitions: [subagentResponse, subagentResponse],
        },
      }),
    ).toEqual(
      err(
        new Error('task execution snapshot contains duplicate subagent keys'),
      ),
    )
  })

  it('uses the fixed snapshot tool and closes its MCP client', async () => {
    const client = {
      callExecutionTool: vi.fn(() =>
        Promise.resolve(
          ok({
            digest: 'synthetic-digest',
            definition: {
              agent_config: {
                agents_md: '',
                skills: {},
                agent_graph: 'phases: []',
              },
              subagent_definitions: [],
            },
          }),
        ),
      ),
      close: vi.fn(() => Promise.resolve()),
    }
    const createMcpClient = vi.fn(() => client)
    const fetchSnapshot = createTaskExecutionSnapshotFetcher(createMcpClient)
    const result = await fetchSnapshot('strategy-synthetic', 'task-synthetic')

    expect(result).toEqual(
      ok({
        agentConfig: { agentsMd: '', skills: {}, agentGraph: 'phases: []' },
        subagentDefinitions: new Map(),
      }),
    )
    expect(createMcpClient.mock.calls).toEqual([
      ['strategy-synthetic', 'task-synthetic', {}],
    ])
    expect(client.callExecutionTool.mock.calls).toEqual([
      [{ name: 'read_task_execution_snapshot', arguments: {} }],
    ])
    expect(client.close.mock.calls).toEqual([[]])
  })
})

describe('createSnapshotSubagentDefinitionFetcher', () => {
  it('uses only captured child definitions when a snapshot exists', async () => {
    const fallback = vi.fn(() => okAsync(subagentDefinition))
    const fetchDefinition = createSnapshotSubagentDefinitionFetcher(
      {
        agentConfig: {
          agentsMd: '',
          skills: {},
          agentGraph: 'phases: []',
        },
        subagentDefinitions: new Map([
          ['helper-synthetic', subagentDefinition],
        ]),
      },
      fallback,
    )

    const result = await fetchDefinition('helper-synthetic')

    expect(result).toEqual(ok(subagentDefinition))
    expect(fallback.mock.calls).toEqual([])
  })
})
