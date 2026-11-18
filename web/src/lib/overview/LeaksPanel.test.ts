import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import LeaksPanel from '#lib/overview/LeaksPanel.svelte';
import { promptHref } from '#lib/shared/links.ts';
import { leakOut, leaks } from '#lib/testing/builders.ts';

const NOW = new Date('2026-11-18T12:00:00Z');
const html = (board: ReturnType<typeof leaks>) =>
	render(LeaksPanel, { props: { leaks: board, now: NOW } })
		.body.replace(/<!--.*?-->/g, '')
		.replace(/\s+/g, ' ');

describe('the possible leaks on the Overview', () => {
	it('is where Worth a look and the key figure lead', () => {
		expect(html(leaks())).toContain('id="leaks"');
	});

	it('says All clear, and what was checked, without findings', () => {
		const out = html(leaks({ checked: { prompts: 1204, kinds: 28 } }));
		expect(out).toContain('All clear');
		expect(out).toContain('1,204 prompts checked for 28 kinds of secret and personal data');
		expect(out).not.toContain('Dismissed');
	});

	it('groups the findings by kind, masked, each with its turn and its two actions', () => {
		const out = html(
			leaks({
				open: [
					leakOut({ id: 'a', kind: 'JWT', masked: 'eyJ…1', prompt: 7 }),
					leakOut({ id: 'b', kind: 'email address', masked: 'j…@x.io', place: 'answer' }),
					leakOut({ id: 'c', kind: 'JWT', masked: 'eyJ…2', place: 'tool_output' })
				]
			})
		);
		expect(out).not.toContain('All clear');
		expect(out.indexOf('JWT')).toBeLessThan(out.indexOf('Email address'));
		expect(out).toContain('eyJ…1');
		expect(out).toContain('in a tool’s output');
		expect(out).toContain('j…@x.io');
		expect(out).toContain(`href="${promptHref(7)}"`);
		expect(out.match(/>Not sensitive</g)).toHaveLength(3);
		expect(out.match(/>Hide</g)).toHaveLength(3);
	});

	it('shows each hidden value as one quiet line, even when all is clear', () => {
		const out = html(
			leaks({ hidden: [leakOut({ decided: 'hidden', decided_at: '2026-11-16T09:00:00Z' })] })
		);
		expect(out).toContain('All clear');
		expect(out).toContain('Hidden on 16 Nov');
	});

	it('folds the dismissed ones under "Dismissed · N", each with Undo', () => {
		const out = html(
			leaks({
				dismissed: [
					leakOut({ id: 'd1', masked: 'one@x.io', decided: 'not_sensitive' }),
					leakOut({ id: 'd2', masked: 'two@x.io', decided: 'not_sensitive' })
				]
			})
		);
		expect(out).toContain('Dismissed · 2');
		expect(out).toContain('one@x.io');
		expect(out.match(/>Undo</g)).toHaveLength(2);
		expect(out).not.toContain('Not sensitive<');
	});
});
