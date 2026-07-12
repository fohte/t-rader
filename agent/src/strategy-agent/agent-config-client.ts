export interface AgentConfig {
  readonly agentsMd: string
  readonly skills: Readonly<Record<string, string>>
  readonly model: string
  readonly smallModel: string
}

interface AgentConfigResponseBody {
  agents_md: string
  skills: Record<string, string>
  model: string
  small_model: string
}

export type FetchAgentConfig = (strategyId: string) => Promise<AgentConfig>

export const createAgentConfigFetcher = (
  backendApiBaseUrl: string,
): FetchAgentConfig => {
  return async (strategyId) => {
    const res = await fetch(
      `${backendApiBaseUrl}/api/strategies/${strategyId}/agent-config`,
    )
    if (!res.ok) {
      throw new Error(
        `failed to fetch agent config for strategy ${strategyId}: ${String(res.status)}`,
      )
    }
    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- backend's documented AgentConfigResponse shape (backend/src/models/strategy.rs)
    const body = (await res.json()) as AgentConfigResponseBody
    return {
      agentsMd: body.agents_md,
      skills: body.skills,
      model: body.model,
      smallModel: body.small_model,
    }
  }
}
