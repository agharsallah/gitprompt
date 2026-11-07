import { AppWindow, SquareTerminal } from '@lucide/svelte';
import { describe, expect, it } from 'vitest';
import { sentBy, surfaceIcon, surfaceIn } from '#lib/prompts/surface.ts';

const claude = {
	id: 'claude-code',
	name: 'Claude Code',
	label: 'Claude Code',
	assistant: 'Claude'
};

describe('surface', () => {
	it('draws the icon the spec names, and an app window for an unknown one', () => {
		expect(surfaceIcon({ icon: 'terminal' })).toBe(SquareTerminal);
		expect(surfaceIcon({ icon: 'hologram' })).toBe(AppWindow);
	});

	it('says who sent a prompt and where it was typed, when known', () => {
		const desktop = { id: 'desktop', name: 'Desktop app', icon: 'app-window' };
		expect(sentBy(claude, desktop)).toBe('Sent by Claude Code, typed in the Desktop app');
		expect(sentBy(claude, null)).toBe('Sent by Claude Code');
		expect(surfaceIn({ id: 'vscode', name: 'VS Code' })).toBe('VS Code');
		expect(surfaceIn({ id: 'cli', name: 'CLI' })).toBe('the CLI');
	});
});
