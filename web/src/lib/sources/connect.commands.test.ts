import { describe, expect, it, vi } from 'vitest';
import { commands } from '#lib/palette/commands.ts';
import { sourcesHref } from '#lib/shared/links.ts';

const go = vi.hoisted(() => vi.fn());
vi.mock('$app/navigation', () => ({ goto: go }));

describe('the "Connect a tool" command', () => {
	it('opens the page where each tool is set up', async () => {
		await import('#lib/sources/connect.commands.ts');
		const command = commands().find((c) => c.id === 'sources.connect');
		await command?.run();
		expect(go).toHaveBeenCalledWith(sourcesHref());
	});
});
