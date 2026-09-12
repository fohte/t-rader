// id の初出順に "<prefix-N>" ラベルを振る。同じ id には同じラベルを返す。
export const createFirstOccurrenceLabeler = (
  prefix: string,
): ((id: string) => string) => {
  const labels = new Map<string, string>()
  return (id) => {
    let label = labels.get(id)
    if (label === undefined) {
      label = `<${prefix}-${String(labels.size + 1)}>`
      labels.set(id, label)
    }
    return label
  }
}
