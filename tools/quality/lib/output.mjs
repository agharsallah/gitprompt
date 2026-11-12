// A command's output as it arrives, made readable live: whole lines (a chunk can end mid-line,
// progress bars rewrite a line with \r), without colour codes, and what a cargo command is doing.

/** Terminal colour and cursor codes, and terminal titles. */
const ANSI = /\x1b\[[0-9;?]*[A-Za-z]|\x1b\][^\x07]*\x07/g;

/** Feeds onLine(line) every complete line of the chunks pushed, without colour codes. */
export function lineSplitter(onLine) {
	let rest = '';
	const emit = (line) => {
		const clean = line.replace(ANSI, '').trimEnd();
		if (clean.trim()) onLine(clean);
	};
	return {
		push(chunk) {
			const parts = (rest + chunk.toString()).split(/\r\n|\n|\r/);
			rest = parts.pop();
			for (const part of parts) emit(part);
		},
		end() {
			if (rest) emit(rest);
			rest = '';
		}
	};
}

/**
 * What a cargo command (or a just recipe running one) is doing, from its output: compiling (how
 * many crates so far), waiting for another cargo's lock, which test binary runs and how many tests
 * passed and failed so far. Returns a function line → text, or undefined when the line says nothing new.
 */
export function cargoActivity() {
	let compiled = 0;
	let binary = null;
	let passed = 0;
	let failed = 0;
	const counts = () => `${passed} passed${failed ? `, ${failed} failed` : ''}`;
	return (line) => {
		const compiling = line.match(/^\s*(?:Compiling|Checking) (\S+) v/);
		if (compiling) return `compiling ${compiling[1]} (${(compiled += 1)} crates so far)`;
		if (/Blocking waiting for file lock/.test(line)) return 'waiting for another cargo command to release backend/target';
		const finished = line.match(/^\s*Finished `(\w+)` profile/);
		if (finished) return `built (${compiled} crates compiled)`;
		const running = line.match(/^\s+Running (?:unittests )?(\S+) \(.*?\/deps\/([\w-]+?)-[0-9a-f]+\)/) || line.match(/^\s+Doc-tests (\S+)/);
		if (running) {
			binary = running[2] ? `${running[2]} ${running[1]}` : `${running[1]} doc tests`;
			return `testing ${binary} · ${counts()}`;
		}
		const test = line.match(/^test \S+(?: - .*)? \.\.\. (ok|FAILED|ignored)/);
		if (!test) return undefined;
		if (test[1] === 'ok') passed += 1;
		if (test[1] === 'FAILED') failed += 1;
		return `testing ${binary ?? ''} · ${counts()}`;
	};
}
