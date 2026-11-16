import { describe, expect, it } from 'vitest';
import { PANELS, USAGE_ACTIONS } from '#lib/shared/usage-actions.ts';

describe('the actions that are counted', () => {
	it('are names the daemon accepts: a short dotted lowercase word (service/usage.rs valid_action)', () => {
		for (const action of USAGE_ACTIONS) expect(action).toMatch(/^[a-z][a-z0-9._-]{0,47}$/);
	});

	it('are few enough for one report (the daemon takes 64 names)', () => {
		expect(USAGE_ACTIONS.length).toBeLessThanOrEqual(64);
	});

	it('are each listed once', () => {
		expect(new Set(USAGE_ACTIONS).size).toBe(USAGE_ACTIONS.length);
	});

	it('count every Overview panel as seen and as clicked through', () => {
		for (const panel of PANELS) {
			expect(USAGE_ACTIONS).toContain(`panel.seen.${panel}`);
			expect(USAGE_ACTIONS).toContain(`panel.click.${panel}`);
		}
	});
});
