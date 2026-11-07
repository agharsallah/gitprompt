/** Axis and color-scale math shared by the charts. Pure, unit tested in scale.test.ts. */

/**
 * Clean axis ticks from 0 to at least `max`: steps of 1, 2 or 5 × 10^k, about `count` of them.
 * Returns [0] for an all-zero series so the baseline still draws.
 */
export function niceTicks(max: number, count = 3): number[] {
	if (!(max > 0)) return [0];
	const raw = max / count;
	const magnitude = 10 ** Math.floor(Math.log10(raw));
	const step = [1, 2, 2.5, 5, 10].map((m) => m * magnitude).find((s) => s >= raw)!;
	const ticks: number[] = [];
	for (let v = 0; v < max + step * 0.999; v += step) ticks.push(+v.toPrecision(12));
	return ticks;
}

/** Number of sequential color steps (--chart-seq-1..6); step 0 is "none". */
export const SEQ_STEPS = 6;

/** Sequential step for a value: 0 for nothing, else 1..SEQ_STEPS by share of `max`. */
export function seqStep(value: number, max: number): number {
	if (!(value > 0) || !(max > 0)) return 0;
	return Math.min(SEQ_STEPS, Math.max(1, Math.ceil((value / max) * SEQ_STEPS)));
}

/** Path of a column with a 4px rounded data end and a square baseline (marks spec). */
export function columnPath(x: number, y: number, width: number, height: number, radius = 4): string {
	if (height <= 0) return '';
	const r = Math.min(radius, width / 2, height);
	const bottom = y + height;
	return [
		`M${x},${bottom}`,
		`V${y + r}`,
		`Q${x},${y} ${x + r},${y}`,
		`H${x + width - r}`,
		`Q${x + width},${y} ${x + width},${y + r}`,
		`V${bottom}`,
		'Z'
	].join('');
}

/** Steps a reader can count in, for a time axis in seconds: 1s, 5s, 15s, 1m, 5m, 15m, 1h... */
const TIME_STEPS = [1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 14_400, 21_600, 43_200, 86_400];

/** Axis ticks from 0 to at least `max` seconds, on clock-friendly steps, about `count` of them. */
export function durationTicks(max: number, count = 3): number[] {
	if (!(max > 0)) return [0];
	const step = TIME_STEPS.find((s) => s >= max / count) ?? Math.ceil(max / count / 86_400) * 86_400;
	const ticks: number[] = [];
	for (let v = 0; v < max + step * 0.999; v += step) ticks.push(v);
	return ticks;
}
