// Build progress in the terminal. On a TTY: a live block at the bottom, one line per running report
// (spinner, elapsed against its last duration, step and latest output), and a line per finished
// report above it. Elsewhere (CI, pipes): plain lines when a report starts, changes step or ends.
const TTY = process.stdout.isTTY;
const FRAMES = '⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏';
const paint = (code, text) => (TTY ? `\x1b[${code}m${text}\x1b[0m` : text);
const MARKS = { done: paint(32, '✓'), failed: paint(31, '✗'), skipped: paint(90, '–'), fresh: paint(90, '=') };

export function duration(msTotal) {
	const seconds = Math.round(msTotal / 1000);
	return seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m${String(seconds % 60).padStart(2, '0')}s`;
}

/** `just now`, `12 min ago`, `3 h ago`, `2 days ago`. */
export function ago(timestamp) {
	const minutes = Math.round((Date.now() - timestamp) / 60_000);
	if (minutes < 1) return 'just now';
	if (minutes < 60) return `${minutes} min ago`;
	const hours = Math.round(minutes / 60);
	return hours < 36 ? `${hours} h ago` : `${Math.round(hours / 24)} days ago`;
}

/** `1m12s / ~3m27s`: elapsed, and the last duration when there is one. */
function elapsed(step) {
	const spent = duration(Date.now() - step.started_at);
	return step.expected_ms ? `${spent} / ~${duration(step.expected_ms)}` : spent;
}

/** What a running report does now: waiting, or its step and latest output. */
export function doing(step) {
	if (step.waiting) return step.waiting;
	return [step.detail, step.activity].filter(Boolean).join(' · ');
}

/**
 * Observes build.steps (runner.mjs) until stop(); finished(step, result) prints a result line,
 * kept(step, record) the line of a report kept as it was (its inputs did not change).
 */
export function createProgress(build, width) {
	const total = build.steps.length;
	let done = 0;
	const prefix = () => `[${String(done).padStart(String(total).length)}/${total}]`;
	const view = TTY ? liveBlock(build, width) : plainLines(build, width);
	return {
		kept(step, record) {
			done += 1;
			const headline = record.headline ? `${record.headline.value} ${record.headline.label}` : '';
			view.print(`  ${MARKS.fresh} ${prefix()} ${step.id.padEnd(width)}  ${headline}  ${paint(90, `up to date, built ${ago(record.generated_at)}`)}`);
		},
		finished(step, result) {
			done += 1;
			const line = `  ${MARKS[step.state]} ${prefix()} ${step.id.padEnd(width)}  ${result.line}  ${paint(90, duration(step.finished_at - step.started_at))}`;
			const detail = step.state === 'failed' ? paint(90, `${result.message}\n(full log: ${result.log})`.replace(/^/gm, '      ')) : null;
			view.print([line, detail].filter(Boolean).join('\n'));
		},
		stop: view.stop
	};
}

function liveBlock(build, width) {
	let drawn = 0;
	const above = [];
	// A line longer than the terminal wraps, and the block would no longer be redrawn in place.
	const fit = (text, room) => (text.length > room ? `${text.slice(0, Math.max(0, room - 1))}…` : text);
	const columns = () => (process.stdout.columns || 100) - 1;
	const draw = () => {
		const frame = FRAMES[Math.floor(Date.now() / 100) % FRAMES.length];
		const running = build.steps.filter((s) => s.state === 'running');
		const lines = running.map((step) => {
			const mark = step.waiting ? paint(33, '⧗') : paint(36, frame);
			const head = fit(`${step.id.padEnd(width)}  ${elapsed(step).padEnd(16)}`, columns() - 4);
			return `  ${mark} ${head}${paint(90, fit(doing(step), columns() - 4 - head.length))}`;
		});
		const queued = build.steps.filter((s) => s.state === 'queued').length;
		const finished = build.steps.length - running.length - queued;
		lines.push(paint(90, fit(`  ${running.length} running · ${queued} queued · ${finished} of ${build.steps.length} done · ${duration(Date.now() - build.started_at)}`, columns())));
		const out = [drawn > 0 ? `\x1b[${drawn}F\x1b[J` : '', ...above.splice(0).map((line) => `${line}\n`), ...lines.map((line) => `${line}\n`)].join('');
		process.stdout.write(out);
		drawn = lines.length;
	};
	const timer = setInterval(draw, 100);
	return {
		print(text) {
			above.push(text);
			draw();
		},
		stop() {
			clearInterval(timer);
			if (drawn > 0) process.stdout.write(`\x1b[${drawn}F\x1b[J`);
			drawn = 0;
			for (const line of above.splice(0)) console.log(line);
		}
	};
}

function plainLines(build, width) {
	const seen = new Map();
	const check = () => {
		for (const step of build.steps.filter((s) => s.state === 'running')) {
			const now = step.waiting || step.detail;
			if (!seen.has(step.id)) console.log(`  … ${step.id.padEnd(width)}  started`);
			if (now && seen.get(step.id) !== now) console.log(`      ${step.id}: ${now}`);
			seen.set(step.id, now);
		}
	};
	const timer = setInterval(check, 500);
	return {
		print(text) {
			check();
			console.log(text);
		},
		stop() {
			clearInterval(timer);
		}
	};
}
