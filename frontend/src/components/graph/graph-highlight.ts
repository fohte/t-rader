export const GRAPH_HIGHLIGHT_CLASS = 'opacity-100'
export const GRAPH_DIMMED_CLASS = 'opacity-30 transition-opacity'

export function getGraphHighlightClass(
  id: string,
  highlightedNodeIds: ReadonlySet<string> | null,
): string {
  return highlightedNodeIds == null || highlightedNodeIds.has(id)
    ? GRAPH_HIGHLIGHT_CLASS
    : GRAPH_DIMMED_CLASS
}
