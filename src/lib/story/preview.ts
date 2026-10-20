// The Content screen's preview, pure: what it is set with, as the engine
// applies it.

export interface PreviewInputs {
  /** Variable (`rel.friend`) → the value to preview with. */
  setup: Record<string, string>;
  replies: string[];
}

/** The setup as the engine applies it: one effect sentence per value. */
export function setupSentences(setup: Record<string, string>): string[] {
  return Object.entries(setup).map(([variable, value]) => `${variable} = ${value}`);
}
