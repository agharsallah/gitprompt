// The ledger (ledger.json): the blocking time dependencies accepted so far, per file and rule, each
// by its line of code (whitespace collapsed), so moving code around never matters while swapping
// one sleep for another, or changing an accepted line, does. Report-only findings are not in it.
// `--update` drops lines that are gone; only a person adds one, by editing the file (the hooks ask
// first).

/** The ledger's path, from the repository root. */
export const LEDGER = 'tools/timing/ledger.json';

/** A line as the ledger keeps it: trimmed, runs of whitespace as one space. */
export const lineKey = (text) => text.trim().replace(/\s+/g, ' ');

/** The ledger of `findings`' blocking ones: { [file]: { [rule]: [line, …] } }, all sorted. */
export function ledgerOf(findings) {
	const ledger = {};
	const blocking = findings.filter((f) => f.policy === 'block');
	for (const f of [...blocking].sort((a, b) => a.file.localeCompare(b.file) || a.rule.localeCompare(b.rule))) {
		((ledger[f.file] ??= {})[f.rule] ??= []).push(lineKey(f.text));
	}
	for (const rules of Object.values(ledger)) for (const lines of Object.values(rules)) lines.sort();
	return ledger;
}

/**
 * Blocking findings against the ledger: `added`, the findings no accepted line covers (each
 * accepted line covers one finding with the same code in that file and rule), and `gone`, accepted
 * lines no longer found ([{ file, rule, text }]): the ledger can drop them.
 */
export function compare(findings, ledger) {
	const left = structuredClone(ledger);
	const added = [];
	for (const f of findings.filter((f) => f.policy === 'block')) {
		const lines = left[f.file]?.[f.rule] ?? [];
		const at = lines.indexOf(lineKey(f.text));
		if (at === -1) added.push(f);
		else lines.splice(at, 1);
	}
	const gone = Object.entries(left).flatMap(([file, rules]) => Object.entries(rules).flatMap(([rule, lines]) => lines.map((text) => ({ file, rule, text }))));
	return { added, gone };
}

/** The ledger without the lines that are gone; never adds one. */
export function shrink(ledger, findings) {
	const kept = ledgerOf(findings);
	const shrunk = {};
	for (const [file, rules] of Object.entries(ledger)) {
		for (const [rule, lines] of Object.entries(rules)) {
			const found = [...(kept[file]?.[rule] ?? [])];
			// Each line found keeps one accepted line with the same code.
			const still = lines.filter((line) => {
				const at = found.indexOf(line);
				if (at === -1) return false;
				found.splice(at, 1);
				return true;
			});
			if (still.length) (shrunk[file] ??= {})[rule] = still;
		}
	}
	return shrunk;
}
