import { describe, expect, it } from 'vitest';
import {
	diffstatText,
	markerSummary,
	phaseReason,
	projectId,
	railOutcome,
	span,
	visible
} from '#lib/stories/story.ts';
import { timeOfDay } from '#lib/shared/format.ts';

describe('projectId', () => {
	it('matches the daemon: base64url without padding', () => {
		// Python: base64.urlsafe_b64encode(repo.encode()).rstrip(b"=")
		expect(projectId('/r/app')).toBe('L3IvYXBw');
		expect(projectId('/tmp/a b/ü')).toBe('L3RtcC9hIGIvw7w');
		expect(projectId('??>>')).toBe('Pz8-Pg');
	});
});

describe('markerSummary', () => {
	it('counts each kind, one by its label and many by its plural', () => {
		const kinds = ['task_notification', 'task_notification', 'subagent_prompt'] as const;
		expect(markerSummary(kinds.map((kind) => ({ kind })))).toBe(
			'2 task notifications, 1 subagent started'
		);
		expect(markerSummary([])).toBe('');
	});
});

describe('diffstatText', () => {
	it('puts signs on the numbers', () => {
		expect(diffstatText({ files: 4, insertions: 1200, deletions: 8 })).toBe('+1,200 −8 in 4 files');
		expect(diffstatText({ files: 0, insertions: 0, deletions: 0 })).toBe('');
		expect(diffstatText(null)).toBe('');
	});
});

describe('phaseReason', () => {
	it('says why a phase began', () => {
		expect(phaseReason({ reason: 'start', gap_s: null, branches: [] })).toBe('First prompts');
		expect(phaseReason({ reason: 'gap', gap_s: 4 * 3600 + 120, branches: [] })).toBe(
			'After a 4h 02m pause'
		);
		expect(phaseReason({ reason: 'branch', gap_s: null, branches: ['feat/x', 'main'] })).toBe(
			'Moved to feat/x'
		);
	});
});

describe('railOutcome', () => {
	it('draws superseded turns as open nodes', () => {
		expect(railOutcome('superseded')).toBe('open');
		expect(railOutcome('failed')).toBe('failed');
	});
});

describe('visible', () => {
	it('shows whole pages, at least one', () => {
		const items = Array.from({ length: 95 }, (_, i) => i);
		expect(visible(items, 0, 40)).toHaveLength(40);
		expect(visible(items, 2, 40)).toHaveLength(80);
		expect(visible(items, 3, 40)).toHaveLength(95);
	});
});

describe('span', () => {
	it('names the day once when it does not change', () => {
		const first = new Date(2026, 10, 5, 10, 23).toISOString();
		const sameDay = new Date(2026, 10, 5, 18, 40).toISOString();
		const nextDay = new Date(2026, 10, 6, 15, 43).toISOString();
		// times are in the viewer's locale
		expect(span(first, sameDay)).toBe(`5 Nov, ${timeOfDay(first)} to ${timeOfDay(sameDay)}`);
		expect(span(first, nextDay)).toBe(`5 Nov, ${timeOfDay(first)} to 6 Nov, ${timeOfDay(nextDay)}`);
		expect(span(first, first)).toBe(span(first, null));
		expect(span(null, null)).toBe('');
	});
});
