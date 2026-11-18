import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { CommitGroup, CommitGrouping } from '#lib/api/types.ts';
import CommitGroups from '#lib/projects/CommitGroups.svelte';
import CommitSummary from '#lib/projects/CommitSummary.svelte';
import { group, trace } from '#lib/testing/commits.ts';

const NOW = new Date('2026-11-17T12:00:00+01:00');

const draw = (groups: CommitGroup[], by: CommitGrouping = 'phase', remote?: string) =>
	render(CommitGroups, { props: { groups, by, remote, now: NOW } }).body;

/** The markup of the row anchored at `commit-<hash>`. */
function rowOf(html: string, hash: string): string {
	const start = html.indexOf(`id="commit-${hash}"`);
	expect(start).toBeGreaterThan(-1);
	return html.slice(html.lastIndexOf('<li', start), html.indexOf('</li>', start));
}

describe('the three kinds of row', () => {
	const traced = trace('traced');
	const likely = trace('likely');
	const hand = trace('by_hand');
	const html = draw([group('1', [traced, likely, hand])]);

	it('a traced commit: hash, subject, change, then its prompt under it and links to it', () => {
		const row = rowOf(html, traced.commit.hash);
		const at = (text: string) => row.indexOf(text);
		expect(at(traced.commit.hash)).toBeLessThan(at(traced.commit.subject));
		expect(at(traced.commit.subject)).toBeLessThan(at('+12 −3 in 2 files'));
		expect(row).toContain(`href="/prompts?id=${traced.prompt?.id}"`);
		expect(row).toContain('title="Session › This prompt"');
		expect(row).toContain(`Wire the uploader`);
		expect(row).not.toContain('and test it');
		expect(row).toContain('Finished:');
		expect(row).not.toContain('likely');
		expect(row).not.toContain('by hand');
	});

	it('a likely commit says so, next to its prompt', () => {
		const row = rowOf(html, likely.commit.hash);
		expect(row).toContain('>likely</span>');
		expect(row).toContain(`href="/prompts?id=${likely.prompt?.id}"`);
		expect(row).not.toContain('· or');
	});

	it('a likely commit names the other prompt that could own it, when there is one', () => {
		const other = trace('traced').prompt;
		if (!other) throw new Error('a traced commit has a prompt');
		const unsure = trace('likely', { alternative: { prompt: other, session: 'session-b' } });
		const row = rowOf(draw([group('1', [unsure])]), unsure.commit.hash);
		expect(row).toMatch(/· or <a href="\/prompts\?id=\d+"[^>]*>Wire the uploader/);
		expect(row).toContain(`href="/prompts?id=${other.id}"`);
	});

	it('a commit made by hand is muted and tagged, with no prompt to open', () => {
		const row = rowOf(html, hand.commit.hash);
		expect(row).toMatch(/class="row hand/);
		expect(row).toContain('by hand');
		expect(row).not.toContain('/prompts?id=');
	});

	it('links the hash to the remote only when there is one', () => {
		expect(rowOf(html, traced.commit.hash)).not.toContain('/commit/');
		const linked = draw([group('1', [traced])], 'phase', 'https://example.test/o/app');
		expect(rowOf(linked, traced.commit.hash)).toContain(
			`href="https://example.test/o/app/commit/${traced.commit.hash}"`
		);
	});
});

describe('grouping', () => {
	it('heads each group with its phase and how many commits it holds, in the order given', () => {
		const html = draw([group('3', [trace(), trace()]), group(null, [trace('by_hand')])]);
		expect(html.indexOf('Phase 3')).toBeLessThan(html.indexOf('Made by hand'));
		expect(html).toMatch(/Phase 3 <span[^>]*>· 2 commits/);
		expect(html).toContain('· 1 commit<');
	});

	it('by branch names the branch, by day the day', () => {
		expect(draw([group('feat/tabs', [trace()])], 'branch')).toContain('feat/tabs');
		expect(draw([group('2026-11-16', [trace()])], 'day')).toContain('Yesterday');
	});
});

describe('the opening sentence', () => {
	it('is three filters, the applied one pressed', () => {
		const counts = { total: 212, traced: 180, likely: 7, by_hand: 25 };
		const html = render(CommitSummary, { props: { counts, show: 'hand' } }).body;
		expect(html).toMatch(/aria-pressed="false"[^>]*>212 commits/);
		expect(html).toMatch(/aria-pressed="false"[^>]*>187 followed a prompt/);
		expect(html).toMatch(/aria-pressed="true"[^>]*>25 by hand/);
	});
});

describe('copying the prompts behind a branch', () => {
	const copy = () => {};
	const drawWith = (groups: CommitGroup[], by: CommitGrouping) =>
		render(CommitGroups, { props: { groups, by, now: NOW, copy } }).body;

	it('is offered on each branch heading, not on commits made by hand', () => {
		const groups = [group('feat/x', [trace('traced')]), group(null, [trace('by_hand')])];
		const html = drawWith(groups, 'branch');
		expect(html).toContain('Copy prompts behind feat/x');
		expect(html.match(/Copy prompts behind/g)).toHaveLength(1);
	});

	it('is not offered when the commits are grouped by phase', () => {
		expect(drawWith([group('1', [trace('traced')])], 'phase')).not.toContain('Copy prompts');
	});
});
