function iconSearchTerms(value: string) {
  const normalized = value.trim().toLocaleLowerCase();
  if (normalized.length < 3) return [normalized];
  const terms = new Set<string>();
  for (let index = 0; index <= normalized.length - 3; index += 1) {
    terms.add(normalized.slice(index, index + 3));
  }
  return [...terms];
}

class IconSearchIndex {
  readonly #names: readonly string[];
  readonly #normalizedNames: readonly string[];
  readonly #postings = new Map<string, number[]>();

  constructor(names: readonly string[]) {
    this.#names = names;
    this.#normalizedNames = names.map((name) => name.toLocaleLowerCase());
    this.#normalizedNames.forEach((name, iconIndex) => {
      for (const term of iconSearchTerms(name)) {
        const posting = this.#postings.get(term) ?? [];
        posting.push(iconIndex);
        this.#postings.set(term, posting);
      }
    });
  }

  search(query: string) {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    if (!normalizedQuery) return this.#names;
    if (normalizedQuery.length < 3) {
      return this.#names.filter((_, index) =>
        this.#normalizedNames[index]?.includes(normalizedQuery)
      );
    }

    const postings = iconSearchTerms(normalizedQuery)
      .map((term) => this.#postings.get(term) ?? [])
      .sort((left, right) => left.length - right.length);
    if (postings.length === 0 || postings[0]?.length === 0) return [];
    const remaining = postings.slice(1).map((posting) => new Set(posting));
    return (postings[0] ?? [])
      .filter(
        (iconIndex) =>
          remaining.every((posting) => posting.has(iconIndex)) &&
          this.#normalizedNames[iconIndex]?.includes(normalizedQuery)
      )
      .map((iconIndex) => this.#names[iconIndex])
      .filter((name): name is string => name !== undefined);
  }
}

export { IconSearchIndex, iconSearchTerms };
