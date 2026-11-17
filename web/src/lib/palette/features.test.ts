import { describe, expect, it, vi } from 'vitest';
import { commands } from '#lib/palette/commands.ts';
import { featureCommands } from '#lib/palette/features.ts';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

describe('the features register their own commands', () => {
	it('finds each <what>.commands.ts under lib/ and runs it', () => {
		const files = Object.keys(featureCommands);
		expect(files).toEqual(
			expect.arrayContaining([
				'/src/lib/layout/pages.commands.ts',
				'/src/lib/layout/theme.commands.ts',
				'/src/lib/sources/connect.commands.ts'
			])
		);
	});

	it('has at least three, from three different features', () => {
		const ids = commands().map((c) => c.id);
		expect(ids).toEqual(expect.arrayContaining(['theme.toggle', 'sources.connect', 'go./prompts']));
		expect(commands().length).toBeGreaterThanOrEqual(3);
	});
});
