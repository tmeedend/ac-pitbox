// The survey being made (`survey.rs`), and its outcome.
//
// Here, not in the Maintenance screen, for the reason the repair lives in
// `repairState`: a survey takes tens of seconds on a large library, nothing
// obliges one to stay on the Workshop meanwhile, and the notification stack
// is what keeps it visible - and its result - across pages.
import { errorText } from "$lib/errors";

export const surveyState = $state<{
  running: boolean;
  /** What the last one produced, until closed. */
  result: { cars: number; tracks: number } | null;
  error: string;
}>({ running: false, result: null, error: "" });

/** One at a time: two surveys would read the same library twice for nothing. */
export async function runSurvey(make: () => Promise<[number, number]>): Promise<void> {
  if (surveyState.running) return;
  surveyState.running = true;
  surveyState.result = null;
  surveyState.error = "";
  try {
    const [cars, tracks] = await make();
    surveyState.result = { cars, tracks };
  } catch (e) {
    console.error("survey", e);
    surveyState.error = errorText(e);
  } finally {
    surveyState.running = false;
  }
}

export function dismissSurvey(): void {
  surveyState.result = null;
  surveyState.error = "";
}
