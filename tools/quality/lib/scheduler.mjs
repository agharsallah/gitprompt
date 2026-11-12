// Runs reports side by side. Three rules keep that safe:
//   - at most `jobs` reports at once (each may run a multi-core build, so not one per core);
//   - a report waits for the reports named in its `after` (when they are in the same build);
//   - a measurement (ctx.alone) runs last, alone: once every other report finished or waits to
//     measure too, so its timings never share the CPU (and it never holds up the other reports).
// The longest report (by its last duration) starts first, so the build ends as early as it can.
import { availableParallelism } from 'node:os';

/** Reports at once by default: half the cores, at most 4 (a Rust build alone uses every core). */
export const DEFAULT_JOBS = Math.max(1, Math.min(4, Math.floor(availableParallelism() / 2)));

/** A counting semaphore: acquire() resolves when one of `size` places is free. */
export function semaphore(size) {
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
		release() {
			const next = waiting.shift();
			if (next) next();
			else free += 1;
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
 * Runs every report through `runReport(report, slot)`, which must release the slot once (it may
 * hand it back while it waits, as ctx.alone does). expected(report) is its last duration in ms (or
 * undefined: never built, so it starts first). onWaiting(report, ids) says what a queued report waits
 * for. Resolves once every report finished; a report's failure never stops the others.
 */
export async function schedule(reports, { jobs, expected = () => undefined, runReport, onWaiting = () => {} }) {
	const slots = semaphore(jobs);
	const ids = new Set(reports.map((report) => report.id));
	const finished = new Set();
	const pending = new Set([...reports].sort((a, b) => (expected(b) ?? Infinity) - (expected(a) ?? Infinity)));
	const blockers = (report) => (report.after || []).filter((id) => ids.has(id) && !finished.has(id));
	let active = 0;
	let passing = false;
	let again = false;

	return new Promise((resolve) => {
		const start = (report) => {
			pending.delete(report);
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
		/** Starts what is ready, in order; runs again when a report finished meanwhile. */
		const startReady = async () => {
			if (passing) {
				again = true;
				return;
			}
			passing = true;
			do {
				again = false;
				for (const report of pending) {
					const waits = blockers(report);
					if (waits.length > 0) onWaiting(report, waits);
				}
				for (const report of [...pending]) {
					if (blockers(report).length > 0) continue;
					await slots.acquire();
					start(report);
				}
				// `after` in a loop would wait forever: start the first one anyway.
				if (active === 0 && pending.size > 0) {
					await slots.acquire();
					start(pending.values().next().value);
					again = true;
				}
			} while (again);
			passing = false;
		};
		if (reports.length === 0) resolve();
		else startReady();
	});
}
