/**
 * The data of a stacked column chart (StackedColumns.svelte): ordered columns, each split into the
 * parts of a few named series, stacked bottom-up in the series' order. Pure, tested in
 * stacked.test.ts.
 */

/** One named series of the stack: a key, its legend label and its color token. */
export type Series = { key: string; label: string; color: string };

/** A column's share of one series. */
export type StackPart = { series: string; value: number };

export type StackedColumn = {
	key: string;
	/** Full name, for the readout and the table view ("Thursday 5 November"). */
	label: string;
	/** Short axis label ("Thu"); defaults to label. */
	tick?: string;
	/** The column's total (the sum of its parts). */
	value: number;
	/** In series order, bottom first. */
	parts: StackPart[];
};

/** One painted segment: its series and where it starts and ends, in value units. */
export type Segment = { series: string; from: number; to: number; top: boolean };

/** The segments of a column, bottom-up; empty parts are skipped and the last one is the top. */
export function stackSegments(parts: StackPart[]): Segment[] {
	const segments: Segment[] = [];
	let from = 0;
	for (const part of parts) {
		if (!(part.value > 0)) continue;
		segments.push({ series: part.series, from, to: from + part.value, top: false });
		from += part.value;
	}
	if (segments.length) segments[segments.length - 1].top = true;
	return segments;
}
