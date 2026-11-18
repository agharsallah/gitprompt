import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { Leaks } from '#lib/api/types.ts';
import HideValues from '#lib/leaks/HideValues.svelte';
import { HideValues as Flow } from '#lib/leaks/hide.svelte.ts';
import { leakOut } from '#lib/testing/builders.ts';

const board: Leaks = {
	open: [
		leakOut({ id: 'f1', kind: 'email', masked: 'j•••@example.com', place: 'prompt' }),
		leakOut({ id: 'f2', kind: 'aws_key', masked: 'AKIA••••', place: 'tool_output' })
	],
	hidden: [],
	dismissed: [],
	checked: { prompts: 10, kinds: 26 }
};

/** The component over a flow already loaded (SSR runs no mount), whose saves are recorded. */
async function drawn(findings: string[]) {
	const saved: string[] = [];
	const flow = new Flow({
		findings,
		load: () => Promise.resolve(board),
		hide: (id) => {
			saved.push(id);
			return Promise.resolve({ id, state: 'hidden', updated: 1 });
		}
	});
	await flow.load();
	return { out: render(HideValues, { props: { findings, flow } }).body, saved };
}

describe('HideValues', () => {
	it('lists each value masked, with its kind and where it sits, before anything is hidden', async () => {
		const { out, saved } = await drawn(['f1', 'f2']);
		expect(out).toContain('Hide these values?');
		expect(out).toContain('j•••@example.com');
		expect(out).toContain('email, in your prompt');
		expect(out).toContain('aws_key, in a tool’s output');
		expect(out).toContain('cannot be undone');
		expect(out).toContain('Hide 2 values');
		expect(saved).toEqual([]);
	});

	it('speaks of one value as one', async () => {
		const { out } = await drawn(['f1']);
		expect(out).toContain('Hide this value?');
		expect(out).toContain('Hide it');
	});

	it('says why when the values are gone', async () => {
		const { out } = await drawn(['gone']);
		expect(out).toContain('Nothing hidden');
		expect(out).toContain('no longer open');
	});
});
