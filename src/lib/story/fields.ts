// What every studio form does with an optional key: a field left blank
// writes nothing, so the file holds only what the author set.

/** Sets `key` of `target` to the trimmed `v`, or removes it when blank. */
export function setOptional<T extends object, K extends keyof T>(target: T, key: K, v: string): void {
  if (v.trim()) (target as Record<K, string>)[key] = v.trim();
  else delete target[key];
}
