// What the prompt detail page shows, as plain data: the facts under the prompt text, the figures of
// the turn, when a text folds, and the payload fields the spec did not map. Pure, tested in
// detail.test.ts; the components only lay these out.
import type { Facts, PromptStats, TurnSummary } from '#lib/api/types.ts';
import type { FactsView } from '#lib/prompts/facts.ts';
import { compact, count, duration, integer, plural } from '#lib/shared/format.ts';

/** One fact about the prompt text; `kind` picks its icon. */
export type TextFact = {
	kind: 'size' | 'questions' | 'code' | 'links' | 'mentions';
	text: string;
};

/** Size always, then what the text holds when it holds any: questions, code, links, @mentions. */
export function textFacts(st: PromptStats): TextFact[] {
	const size = `${plural(st.words ?? 0, 'word')}, about ${compact(st.approx_tokens ?? 0)} tokens`;
	const facts: TextFact[] = [{ kind: 'size', text: size }];
	if (st.questions) facts.push({ kind: 'questions', text: plural(st.questions, 'question') });
	if (st.code_blocks) facts.push({ kind: 'code', text: plural(st.code_blocks, 'code block') });
	if (st.urls?.length) facts.push({ kind: 'links', text: plural(st.urls.length, 'link') });
	if (st.at_mentions?.length) {
		facts.push({ kind: 'mentions', text: st.at_mentions.map((m) => `@${m}`).join(' ') });
	}
	return facts;
}

/** One figure of a turn: a value with an alarming `hint`, a quiet `note` and a tooltip `title`. */
export type Figure = {
	label: string;
	value: string;
	hint?: string;
	note?: string;
	title?: string;
};

/** How the turn went in figures: time, tool calls, files, and tokens and cost when reported. */
export function turnFigures(s: TurnSummary, view: FactsView, f: Facts): Figure[] {
	const figures: Figure[] = [];
	if (s.response_s != null) figures.push({ label: 'Worked for', value: duration(s.response_s) });
	figures.push({
		label: 'Tool calls',
		value: integer(count(s.tool_calls)),
		hint: s.tool_failures ? `${s.tool_failures} failed` : undefined
	});
	figures.push({ label: 'Files changed', value: integer(s.files_written?.length ?? 0) });
	if (view.tokens) {
		const reasoning = f.tokens?.reasoning;
		figures.push({
			label: 'Tokens',
			value: view.tokens.total,
			note: reasoning ? `${compact(reasoning)} reasoning` : undefined,
			title: view.tokens.hint
		});
	}
	if (view.cost) figures.push({ label: 'Cost', value: view.cost });
	return figures;
}

const FOLD_CHARS = 1400;
const FOLD_LINES = 18;

/** Whether a text is long enough to fold, so the story around it stays in view. */
export function isLong(text: string | null | undefined): boolean {
	return !!text && (text.length > FOLD_CHARS || text.split('\n').length > FOLD_LINES);
}

/** The fields the spec mapped, without the prompt text (shown above). */
export function capturedFields(fields: object): Record<string, unknown> {
	return Object.fromEntries(Object.entries(fields).filter(([k]) => k !== 'prompt'));
}

/**
 * Everything the tool sent that the spec did not map, so new metadata shows up without code
 * changes: the payload's entries whose value is none of the mapped fields' values.
 */
export function unmappedPayload(
	raw: Record<string, unknown>,
	fields: object
): Record<string, unknown> {
	const mapped = Object.values(fields);
	return Object.fromEntries(Object.entries(raw).filter(([, v]) => !mapped.some((m) => m === v)));
}
