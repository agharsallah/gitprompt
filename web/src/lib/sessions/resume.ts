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

/** What the story says in place of the command: the tool, or "this tool" when it is not named. */
export const noResumeText = (tool?: string | null) =>
	`${tool || 'This tool'} has no resume command`;

/** Count a copy of a resume command (on this machine only, lib/shared/usage.ts). */
export const countResumeCopy = (count: (action: UsageAction) => void = track) =>
	count('copy.resume');
