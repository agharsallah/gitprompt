import { describe, expect, it } from 'vitest';
import type { Origin } from '#lib/api/types.ts';
import { prompt } from '#lib/testing/builders.ts';
import {
	ORIGINS,
	injectedLabel,
	injectedSummary,
	isSystem,
	originOf,
	typedText
} from '#lib/prompts/origin.ts';

describe('origins', () => {
	it('marks only typed prompts and slash commands as yours', () => {
		const yours = Object.entries(ORIGINS)
			.filter(([, o]) => !o.system)
			.map(([k]) => k);
		expect(yours).toEqual(['typed', 'slash_command']);
	});

	it('gives every origin a label and an icon', () => {
		for (const o of Object.values(ORIGINS)) expect(o.label && o.icon).toBeTruthy();
	});

	it('treats a prompt without origin (older daemon) as typed', () => {
		const old = prompt({ origin: undefined as unknown as Origin });
		expect(originOf(old)).toBe(ORIGINS.typed);
		expect(isSystem(old)).toBe(false);
		expect(isSystem(prompt({ origin: 'task_notification' }))).toBe(true);
	});
});

describe('typedText', () => {
	it('prefers the typed text', () =>
		expect(
			typedText(prompt({ prompt: '<ide_selection>x</ide_selection> Fix', typed_text: 'Fix' }))
		).toBe('Fix'));

	it('falls back to the prompt when nothing was typed', () =>
		expect(typedText(prompt({ prompt: '<ide_selection>x</ide_selection>', typed_text: '' }))).toBe(
			'<ide_selection>x</ide_selection>'
		));

	it('is null without text', () => expect(typedText(prompt({ prompt: null }))).toBeNull());
});

describe('injected context', () => {
	it('names known kinds and words the rest', () => {
		expect(injectedLabel('ide_selection')).toBe('IDE selection');
		expect(injectedLabel('local-command-stdout')).toBe('Local command stdout');
	});

	it('summarises one block by kind and several by count', () => {
		expect(injectedSummary([])).toBe('');
		expect(injectedSummary([{ kind: 'ide_selection', chars: 2241 }])).toBe(
			'IDE selection, 2,241 characters'
		);
		expect(
			injectedSummary([
				{ kind: 'ide_selection', chars: 10 },
				{ kind: 'system-reminder', chars: 5 }
			])
		).toBe('2 blocks, 15 characters');
	});
});
