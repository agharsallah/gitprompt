import { describe, expect, it } from 'vitest';
import { clickAction } from '#lib/insights/counting.ts';

describe('clickAction', () => {
	it('counts a link to the prompts under the panel it is in', () => {
		expect(clickAction('/prompts?origin=human&range=7d', 'rhythm')).toBe('panel.click.rhythm');
		expect(clickAction('/prompts', 'key-figures')).toBe('panel.click.key-figures');
		expect(clickAction('http://127.0.0.1:7878/prompts/?starred=true', 'starred')).toBe(
			'panel.click.starred'
		);
	});

	it('counts nothing for a link to another page', () => {
		expect(clickAction('/sessions/abc', 'sessions')).toBeUndefined();
		expect(clickAction('/prompts-old', 'sessions')).toBeUndefined();
		expect(clickAction('/settings#data', 'spend')).toBeUndefined();
	});

	it('counts nothing outside a known panel', () => {
		expect(clickAction('/prompts?q=x', undefined)).toBeUndefined();
		expect(clickAction('/prompts?q=x', 'not-a-panel')).toBeUndefined();
	});

	it('follows the base path the app is served under', () => {
		expect(clickAction('/gp/prompts?q=x', 'rhythm', '/gp/prompts')).toBe('panel.click.rhythm');
		expect(clickAction('/prompts?q=x', 'rhythm', '/gp/prompts')).toBeUndefined();
	});
});
