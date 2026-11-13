// Summaries of repeated timings: a run is noisy, the median of a few is not.

/** The value under which `share` (0..1) of the values fall (nearest rank); null when empty. */
export function percentile(values, share) {
	if (values.length === 0) return null;
	const sorted = [...values].sort((a, b) => a - b);
	return sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(share * sorted.length) - 1))];
}

export const median = (values) => percentile(values, 0.5);

/** { median, max, runs } of the timings; median and max null when there are none. */
export function summary(values) {
	return { median: median(values), max: values.length ? Math.max(...values) : null, runs: values.length };
}

/** Runs fn(i) `times` times, one after another, and resolves the results in order. */
export async function repeat(times, fn) {
	const results = [];
	for (let i = 0; i < times; i += 1) results.push(await fn(i));
	return results;
}
