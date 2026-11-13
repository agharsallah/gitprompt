// Runs reports side by side. Rules that keep it safe and quick:
//   - at most `jobs` long reports at once (each may run a multi-core build, so not one per core);
//   - quick reports (under QUICK_MS last time) run in a lane of their own, shortest first, so
//     their results land within seconds instead of queueing behind builds;
//   - a report the others wait for (their `after`) starts first, then the longest one, so the build
//     ends as early as it can;
//   - a report waits for the reports named in its `after` (when they are in the same build);
//   - a measurement (ctx.alone) runs last, alone: once every other report finished or waits to
//     measure too, so its timings never share the CPU (and it never holds up the other reports).
import { availableParallelism } from 'node:os';

/** Long reports at once by default: half the cores, at most 4 (a Rust build alone uses every core). */
export const DEFAULT_JOBS = Math.max(1, Math.min(4, Math.floor(availableParallelism() / 2)));
/** A report that took less than this last time is quick: it runs in the quick lane. */
export const QUICK_MS = 30_000;
/** Quick reports at once, besides the long ones. */
export const QUICK_JOBS = 2;

/**
 * A counting semaphore of `size` places. acquire() resolves when one is free; tryAcquire() takes one
 * only if free now. onFree() runs when a place frees up and nobody waits for it.
 */
export function semaphore(size, onFree = () => {}) {
	let free = size;
	const waiting = [];
	return {
		acquire() {
			if (free > 0) {
				free -= 1;
				return Promise.resolve();
			}
			return new Promise((resolve) => waiting.push(resolve));
		},
		tryAcquire() {
			if (free === 0) return false;
			free -= 1;
			return true;
		},
		release() {
			const next = waiting.shift();
			if (next) next();
			else {
				free += 1;
				onFree();
			}
		}
	};
}

/**
 * Commands share the gate; a measurement holds it alone. shared(fn) runs at once unless a
 * measurement runs; alone(fn) waits until no shared command runs and idle() says no other work is
 * left, then runs with none starting. Call poke() when idle() may have changed.
 */
export function gate(idle = () => true) {
	let readers = 0;
	let writer = false;
	const readersWaiting = [];
	const writersWaiting = [];
	const pump = () => {
		if (writer) return;
		if (readersWaiting.length > 0) {
			readers += readersWaiting.length;
			for (const resolve of readersWaiting.splice(0)) resolve();
			return;
		}
		if (readers === 0 && writersWaiting.length > 0 && idle()) {
			writer = true;
			writersWaiting.shift()();
		}
	};
	const enter = (queue) => {
		const entered = new Promise((resolve) => queue.push(resolve));
		pump();
		return entered;
	};
	return {
		/** True while a measurement runs: shared(fn) would wait. */
		measuring: () => writer,
		poke: pump,
		async shared(fn) {
			await enter(readersWaiting);
			try {
				return await fn();
			} finally {
				readers -= 1;
				pump();
			}
		},
		async alone(fn) {
			await enter(writersWaiting);
			try {
				return await fn();
			} finally {
				writer = false;
				pump();
			}
		}
	};
}

/**
 * The order reports start in: the ones others wait for, then the quick ones (shortest first), then
 * the long ones (longest first; never built counts as longest).
 */
export function startOrder(reports, expected = () => undefined, quickMs = QUICK_MS) {
	const awaited = new Set(reports.flatMap((report) => report.after || []));
	const quick = (report) => (expected(report) ?? Infinity) < quickMs;
	const rank = (report) => [awaited.has(report.id) ? 0 : 1, quick(report) ? 0 : 1, quick(report) ? expected(report) : -(expected(report) ?? Number.MAX_SAFE_INTEGER)];
	const compare = (a, b) => {
		const [x, y] = [rank(a), rank(b)];
		return x[0] - y[0] || x[1] - y[1] || x[2] - y[2];
	};
	return [...reports].sort(compare);
}

/**
 * Runs every report through `runReport(report, slots)`, which must release the slot once (it may
 * hand it back while it waits, as ctx.alone does). expected(report) is its last duration in ms (or
 * undefined: never built). onWaiting(report, ids) says what a queued report waits for. Resolves once
 * every report finished; a report's failure never stops the others.
 */
export function schedule(reports, { jobs, quickJobs = QUICK_JOBS, quickMs = QUICK_MS, expected = () => undefined, runReport, onWaiting = () => {} }) {
	const ids = new Set(reports.map((report) => report.id));
	const finished = new Set();
	const pending = startOrder(reports, expected, quickMs);
	const isQuick = (report) => (expected(report) ?? Infinity) < quickMs;
	const blockers = (report) => (report.after || []).filter((id) => ids.has(id) && !finished.has(id));
	let active = 0;

	return new Promise((resolve) => {
		const lanes = { long: semaphore(jobs, () => startReady()), quick: semaphore(quickJobs, () => startReady()) };
		/** Starts each ready report `lane(report)` gives a place to. */
		const fill = (lane) => {
			for (const report of [...pending]) {
				if (blockers(report).length > 0) continue;
				const slots = lane(report);
				if (slots?.tryAcquire()) start(report, slots);
			}
		};
		const start = (report, slots) => {
			pending.splice(pending.indexOf(report), 1);
			active += 1;
			Promise.resolve()
				.then(() => runReport(report, slots))
				.catch(() => {})
				.finally(() => {
					active -= 1;
					finished.add(report.id);
					if (finished.size === reports.length) resolve();
					else startReady();
				});
		};
		/**
		 * Starts every ready report a place is free for, in order: each report in its own lane, then
		 * quick reports in long places nobody else needs (a long report never waits for a quick one).
		 */
		function startReady() {
			for (const report of pending) {
				const waits = blockers(report);
				if (waits.length > 0) onWaiting(report, waits);
			}
			fill((report) => (isQuick(report) ? lanes.quick : lanes.long));
			fill((report) => (isQuick(report) ? lanes.long : null));
			// `after` in a loop would wait forever: start the first one anyway.
			if (active === 0 && pending.length > 0) {
				lanes.long.tryAcquire();
				start(pending[0], lanes.long);
			}
		}
		if (reports.length === 0) resolve();
		else startReady();
	});
}
