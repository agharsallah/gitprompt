import { describe, expect, it } from 'vitest';
import { platform, platformFor } from '#lib/platform/index.ts';

const page = {
	document: {
		createElement: () => ({}),
		visibilityState: 'visible',
		body: { append: () => {} },
		addEventListener: () => {},
		removeEventListener: () => {}
	},
	url: { createObjectURL: () => 'blob:x', revokeObjectURL: () => {} }
} as unknown as Parameters<typeof platformFor>[1];

describe('platformFor', () => {
	it('answers as the browser where the shell is not in the page', () => {
		expect(platformFor({}, page).desktop).toBe(false);
	});
});

describe('platform', () => {
	it('is the browser in a test, and each answer is the chosen one', async () => {
		expect(platform.desktop).toBe(false);
		await expect(platform.hideWindow()).resolves.toBeUndefined();
		expect(typeof platform.onNavigate(() => {})).toBe('function');
		await expect(platform.recall.problem()).resolves.toBeNull();
		await expect(platform.recall.register('CmdOrCtrl+Alt+P')).resolves.toEqual({
			kind: 'unsupported'
		});
	});
});
