/**
 * Search hits on the Prompts page: the hits (matching words in context) joined to the prompts the
 * same filters list, so a hit can be copied in full and the quick filters (`?show=`, applied to the
 * loaded prompts in the browser) keep applying. Pure, tested in hits.test.ts.
 */
import type { Prompt, SearchHit } from '#lib/api/types.ts';
import { typedText } from '#lib/prompts/origin.ts';

export type HitRow = {
	hit: SearchHit;
	/** The full text to copy, without the context the tool added; null when none was captured. */
	text: string | null;
};

/** The hits whose prompt is among `prompts`, in the hits' order (newest first). */
export function hitRows(hits: readonly SearchHit[], prompts: readonly Prompt[]): HitRow[] {
	const byId = new Map(prompts.map((p) => [p.id, p]));
	return hits.flatMap((hit) => {
		const prompt = byId.get(hit.id);
		return prompt ? [{ hit, text: typedText(prompt) }] : [];
	});
}
