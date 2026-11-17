import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SessionStory } from '#lib/api/types.ts';
import { actionsFor, type ActionContext } from '#lib/palette/actions.ts';
import { copySessionResume } from '#lib/sessions/resume.commands.ts';
import { sessionStory } from '#lib/testing/builders.ts';
import { toasts } from '#lib/ui/toast.svelte.ts';

const story = vi.hoisted((): { current?: SessionStory } => ({}));
const tracked = vi.hoisted((): string[] => []);
vi.mock('#lib/api/client.ts', () => ({
	api: { sessionStory: () => Promise.resolve(story.current) }
}));
vi.mock('#lib/shared/usage.ts', () => ({ track: (action: string) => tracked.push(action) }));

const context = () => {
	const copied: string[] = [];
	const ctx: ActionContext = {
		navigate: vi.fn(),
		copy: async (text) => copied.push(await text) > 0,
		close: vi.fn(),
		promptText: () => Promise.resolve(''),
		fail: vi.fn()
	};
	return { ctx, copied };
};

const resume = {
	command: 'claude --resume s1',
	cwd: '/work/my app',
	paste: "cd '/work/my app' && claude --resume s1"
};

describe('copy resume in the palette (⌥↵ on a session)', () => {
	beforeEach(() => {
		tracked.length = 0;
		toasts.items = [];
	});

	it('is a session action on ⌥↵', () => {
		expect(actionsFor('session').map((a) => [a.key, a.label])).toContainEqual([
			'alt+enter',
			'copy resume'
		]);
	});

	it('copies the line the row and the story copy, and counts it', async () => {
		story.current = sessionStory({ resume });
		const { ctx, copied } = context();
		await copySessionResume('s1', ctx);
		expect(copied).toEqual([resume.paste]);
		expect(tracked).toEqual(['copy.resume']);
	});

	it('says the tool has no resume command, copies nothing and closes', async () => {
		const cursor = { id: 'cursor', name: 'Cursor', label: 'Cursor', assistant: 'Cursor' };
		story.current = sessionStory({ connector: cursor });
		const { ctx, copied } = context();
		await copySessionResume('s1', ctx);
		expect(copied).toEqual([]);
		expect(ctx.close).toHaveBeenCalled();
		expect(toasts.items.map((t) => t.message)).toEqual(['Cursor has no resume command']);
		expect(tracked).toEqual([]);
	});
});
