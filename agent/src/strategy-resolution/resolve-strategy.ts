export interface StrategyCandidate {
  readonly strategyId: string
  readonly name: string
}

export type StrategyResolution =
  | { readonly kind: 'resolved'; readonly strategyId: string }
  | {
      readonly kind: 'ambiguous'
      readonly candidates: readonly StrategyCandidate[]
    }
  | {
      readonly kind: 'not_found'
      readonly candidates: readonly StrategyCandidate[]
    }

// A candidate must cover at least this fraction of its name (by longest
// contiguous run of shared characters) to be considered a plausible match at
// all.
const MIN_MATCH_SCORE = 0.5
// Among plausible candidates, the top match must beat the runner-up by this
// much to be treated as unambiguous. Strategy names sharing a common suffix
// (e.g. "長期投資" / "中期投資") otherwise both clear MIN_MATCH_SCORE whenever
// the text names only one of them, so a fixed threshold alone can't tell
// "the user picked one of these" from "the user picked the other one".
const MATCH_MARGIN = 0.15

const longestCommonSubstringLength = (a: string, b: string): number => {
  if (a.length === 0 || b.length === 0) return 0
  let prev = new Array<number>(b.length + 1).fill(0)
  let max = 0
  for (let i = 1; i <= a.length; i++) {
    const cur = new Array<number>(b.length + 1).fill(0)
    for (let j = 1; j <= b.length; j++) {
      if (a[i - 1] === b[j - 1]) {
        const run = (prev[j - 1] ?? 0) + 1
        cur[j] = run
        if (run > max) max = run
      }
    }
    prev = cur
  }
  return max
}

// Longest contiguous run of `candidateName` found anywhere in `text`,
// normalized by the candidate name's length. Favors a name appearing as a
// recognizable chunk of free text over one that merely shares a few
// scattered characters with it.
export const scoreStrategyNameMatch = (
  candidateName: string,
  text: string,
): number => {
  const name = candidateName.toLowerCase()
  const haystack = text.toLowerCase()
  if (name.length === 0) return 0
  return longestCommonSubstringLength(name, haystack) / name.length
}

export const resolveStrategy = (
  candidates: readonly StrategyCandidate[],
  text: string,
): StrategyResolution => {
  const scored = candidates
    .map((candidate) => ({
      candidate,
      score: scoreStrategyNameMatch(candidate.name, text),
    }))
    .sort((a, b) => b.score - a.score)
  const plausible = scored.filter((s) => s.score >= MIN_MATCH_SCORE)

  const top = plausible[0]
  if (top === undefined) {
    return { kind: 'not_found', candidates }
  }
  const second = plausible[1]
  if (second === undefined || top.score - second.score >= MATCH_MARGIN) {
    return { kind: 'resolved', strategyId: top.candidate.strategyId }
  }
  return {
    kind: 'ambiguous',
    candidates: plausible.map((s) => s.candidate),
  }
}
