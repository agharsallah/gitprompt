/**
 * Picking a session up again. The daemon builds the line to paste (`resume.paste`: the folder
 * quoted for the shell, then the tool's command), so the page only shows and copies it. These are
 * pure helpers the session row, the session story and the command palette share.
 */
import type { Resume } from '#lib/api/types.ts';
import { track } from '#lib/shared/usage.ts';
import type { UsageAction } from '#lib/shared/usage-actions.ts';

/** Anything that may know how to resume: a session row, a story, a stats group. */
export type Resumable = { resume?: Resume | null };

/** What to paste into a terminal to pick the session up again, or null when there is no command. */
export const resumeText = (session: Resumable): string | null => session.resume?.paste ?? null;

/** Ids longer than this are cut to `SHORT_ID` characters and `…`, like a commit hash. */
const LONG_ID = 12;
const SHORT_ID = 8;

const shortId = (id: string) => (id.length > LONG_ID ? `${id.slice(0, SHORT_ID)}…` : id);

/**
 * The tool's command with the session's own id shortened (`claude --resume 4a7d31a7…`, `copilot
 * --resume=4a7d31a7…`): what a row's button shows, so each row says which session it copies.
 * The id is the command's last word (or the value of its last `--flag=`).
 */
export function resumeLabel(resume: Resume): string {
	const words = resume.command.trim().split(/\s+/);
	if (words.length < 2) return resume.command.trim();
	const last = words.pop() ?? '';
	const eq = last.indexOf('=') + 1;
	return [...words, last.slice(0, eq) + shortId(last.slice(eq))].join(' ');
}

/** What the story says in place of the command: the tool, or "this tool" when it is not named. */
export const noResumeText = (tool?: string | null) =>
	`${tool || 'This tool'} has no resume command`;

/** Count a copy of a resume command (on this machine only, lib/shared/usage.ts). */
export const countResumeCopy = (count: (action: UsageAction) => void = track) =>
	count('copy.resume');
