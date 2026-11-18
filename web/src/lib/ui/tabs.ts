/**
 * Keyboard access to a row of tabs (Tabs.svelte): ← → go to the next enabled tab and round at the
 * ends, Home and End jump to the first and the last. The roving logic is the charts' one
 * (charts/roving.ts), over the enabled tabs only. Pure, tested in tabs.test.ts.
 */
import { rovingStep } from '#lib/ui/charts/roving.ts';

/** The index (in `tabs`) of the tab a key moves to from `current`, or null when the key does nothing. */
export function tabKeyTarget(
	key: string,
	current: number,
	tabs: readonly { disabled?: boolean }[]
): number | null {
	const enabled = tabs.flatMap((t, i) => (t.disabled ? [] : [i]));
	const at = enabled.indexOf(current);
	const step = rovingStep(key, at >= 0 ? at : null, enabled.length, true);
	return step.kind === 'move' ? enabled[step.index] : null;
}
