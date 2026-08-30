class IconComponentCache<T> {
  readonly #limit: number;
  readonly #entries = new Map<string, T>();

  constructor(limit: number) {
    if (!Number.isInteger(limit) || limit <= 0) {
      throw new Error('Icon component cache limit must be a positive integer');
    }
    this.#limit = limit;
  }

  getOrCreate(name: string, create: () => T) {
    const cached = this.#entries.get(name);
    if (cached !== undefined) {
      this.#entries.delete(name);
      this.#entries.set(name, cached);
      return cached;
    }

    const created = create();
    this.#entries.set(name, created);
    while (this.#entries.size > this.#limit) {
      const oldest = this.#entries.keys().next().value;
      if (oldest === undefined) break;
      this.#entries.delete(oldest);
    }
    return created;
  }

  has(name: string) {
    return this.#entries.has(name);
  }

  get size() {
    return this.#entries.size;
  }
}

export { IconComponentCache };
