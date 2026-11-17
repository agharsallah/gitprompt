import { describe, expect, it } from 'vitest';
import type { Resume } from '#lib/api/types.ts';
import { copyResume, countResumeCopy, noResumeText, resumeText } from '#lib/sessions/resume.ts';
import type { UsageAction } from '#lib/shared/usage-actions.ts';

const resume: Resume = {
	command: 'claude --resume abc',
	cwd: "/work/o'neil's app",
	paste: String.raw`cd '/work/o'\''neil'\''s app' && claude --resume abc`
};

/** A clipboard and a counter the test reads back. */
function desk(fail = false) {
	const written: string[] = [];
	const counted: UsageAction[] = [];
	return {
		written,
		counted,
		write: (text: string) =>
			fail ? Promise.reject(new Error('denied')) : Promise.resolve(void written.push(text)),
		count: (action: UsageAction) => void counted.push(action)
	};
}

describe('resumeText', () => {
	it('is the line the daemon built, quoting and all', () => {
		expect(resumeText({ resume })).toBe(resume.paste);
	});

	it('is null when the tool has no resume command', () => {
		expect([resumeText({}), resumeText({ resume: null })]).toEqual([null, null]);
	});
});

describe('noResumeText', () => {
	it('names the tool', () => {
		expect(noResumeText('Cursor')).toBe('Cursor has no resume command');
		expect(noResumeText(undefined)).toBe('This tool has no resume command');
		expect(noResumeText('')).toBe('This tool has no resume command');
	});
});

describe('copyResume', () => {
	it('puts the line on the clipboard and counts one copy', async () => {
		const { write, count, written, counted } = desk();
		expect(await copyResume({ resume }, write, count)).toBe(true);
		expect([written, counted]).toEqual([[resume.paste], ['copy.resume']]);
	});

	it('does nothing for a session without a command', async () => {
		const { write, count, written, counted } = desk();
		expect(await copyResume({ resume: null }, write, count)).toBe(false);
		expect([written, counted]).toEqual([[], []]);
	});

	it('counts nothing when the clipboard refuses', async () => {
		const { write, count, counted } = desk(true);
		expect(await copyResume({ resume }, write, count)).toBe(false);
		expect(counted).toEqual([]);
	});
});

describe('countResumeCopy', () => {
	it('counts copy.resume', () => {
		const { count, counted } = desk();
		countResumeCopy(count);
		expect(counted).toEqual(['copy.resume']);
	});
});
