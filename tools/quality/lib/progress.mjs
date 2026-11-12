// Build progress in the terminal: on a TTY one live line per running report (spinner, current step,
// elapsed time) that turns into a result line; elsewhere (CI, pipes) plain start and end lines.
const TTY = process.stdout.isTTY;
const FRAMES = '⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏';
const paint = (code, text) => (TTY ? `\x1b[${code}m${text}\x1b[0m` : text);
const MARKS = { done: paint(32, '✓'), failed: paint(31, '✗'), skipped: paint(90, '–') };

export function duration(msTotal) {
	const seconds = Math.round(msTotal / 1000);
	return seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m${String(seconds % 60).padStart(2, '0')}s`;
}

export function createProgress(total, width) {
	let line = null;
	let timer = null;
	const label = (index, report) => `[${String(index + 1).padStart(String(total).length)}/${total}] ${report.id.padEnd(width)}`;
	const draw = () => {
		if (!line) return;
		const frame = FRAMES[Math.floor(Date.now() / 100) % FRAMES.length];
		const text = `  ${paint(36, frame)} ${line.label}  ${paint(90, line.step.slice(0, 60))}  ${duration(Date.now() - line.started)}`;
		process.stdout.write(`\r\x1b[2K${text}`);
	};
	return {
		start(index, report) {
			line = { label: label(index, report), step: 'starting', started: Date.now() };
			if (TTY) timer = setInterval(draw, 100);
			else console.log(`  … ${line.label}`);
		},
		step(text) {
			if (!line) return;
			line.step = text;
			if (!TTY) console.log(`      ${text}`);
		},
		end(state, summary) {
			clearInterval(timer);
			const text = `  ${MARKS[state]} ${line.label}  ${summary}  ${paint(90, duration(Date.now() - line.started))}`;
			if (TTY) process.stdout.write(`\r\x1b[2K${text}\n`);
			else console.log(text);
			line = null;
		},
		detail(text) {
			console.log(paint(90, text.replace(/^/gm, '      ')));
		}
	};
}
