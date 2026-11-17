// "Copy resume" (⌥↵) on a session in the palette: the line the session row and story copy.
import { api } from '#lib/api/client.ts';
import { registerAction, type ActionContext } from '#lib/palette/actions.ts';
import { countResumeCopy, noResumeText, resumeText } from '#lib/sessions/resume.ts';
import { toasts } from '#lib/ui/toast.svelte.ts';

/** Copy session `id`'s resume line, or say its tool has none. */
export async function copySessionResume(id: string, ctx: ActionContext): Promise<void> {
	// The copy starts now, on the promise of the line: the clipboard refuses one made after a fetch.
	const line = api.sessionStory(id).then((story) => {
		const text = resumeText(story);
		if (text === null) throw new NoResume(story.connector?.name);
		return text;
	});
	try {
		if (await ctx.copy(line)) countResumeCopy();
	} catch (error) {
		if (!(error instanceof NoResume)) throw error;
		ctx.close();
		toasts.show(noResumeText(error.tool), { tone: 'info' });
	}
}

/** The session's tool has no resume command: there is no line to copy. */
class NoResume extends Error {
	constructor(readonly tool?: string) {
		super('no resume command');
	}
}

registerAction('session', {
	key: 'alt+enter',
	label: 'copy resume',
	run: (item, ctx) => copySessionResume(item.id, ctx)
});
