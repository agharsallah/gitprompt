/**
 * Keyboard access to a row of chart columns: ← → step from column to column (Home and End jump to
 * the ends), ↵ opens the focused one, Escape lets go. One column is in the tab order at a time
 * (roving tabindex), so the row is one tab stop. Pure, tested in roving.test.ts.
 */

export type RovingStep =
	/** Move focus to this column. */
	| { kind: 'move'; index: number }
	/** Open this column (its link, or its selection). */
	| { kind: 'open'; index: number }
	/** Let go of the column (hide its readout). */
	| { kind: 'leave' }
	/** Not a key the row handles: leave the event alone. */
	| { kind: 'none' };

/**
 * What a key does on a row of `count` columns while `current` has focus (null: none yet).
 * The arrows stop at the ends, or go round with `wrap`.
 */
export function rovingStep(
	key: string,
	current: number | null,
	count: number,
	wrap = false
): RovingStep {
	if (!count) return { kind: 'none' };
	const last = count - 1;
	switch (key) {
		case 'ArrowRight':
			return { kind: 'move', index: next(current ?? -1, last, wrap) };
		case 'ArrowLeft':
			return { kind: 'move', index: previous(current ?? count, last, wrap) };
		case 'Home':
			return { kind: 'move', index: 0 };
		case 'End':
			return { kind: 'move', index: last };
		case 'Enter':
			return current === null ? { kind: 'none' } : { kind: 'open', index: current };
		case 'Escape':
			return { kind: 'leave' };
		default:
			return { kind: 'none' };
	}
}

const next = (from: number, last: number, wrap: boolean) =>
	from >= last ? (wrap ? 0 : last) : from + 1;

const previous = (from: number, last: number, wrap: boolean) =>
	from <= 0 ? (wrap ? last : 0) : from - 1;

/** The column in the tab order: the focused one, else `fallback` (e.g. today), else the first. */
export const tabStop = (focused: number | null, fallback: number, count: number) =>
	focused ?? (fallback >= 0 && fallback < count ? fallback : 0);
