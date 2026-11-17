import { describe, expect, it } from 'vitest';
import type { Resume } from '#lib/api/types.ts';
import { countResumeCopy, noResumeText, resumeText } from '#lib/sessions/resume.ts';
import type { UsageAction } from '#lib/shared/usage-actions.ts';

const resume: Resume = {
	command: 'claude --resume abc',
	cwd: "/work/o'neil's app",
	paste: String.raw`cd '/work/o'\''neil'\''s app' && claude --resume abc`
};

/** A counter the test reads back. */
function desk() {
	const counted: UsageAction[] = [];
	return { counted, count: (action: UsageAction) => void counted.push(action) };
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

describe('countResumeCopy', () => {
	it('counts copy.resume', () => {
		const { count, counted } = desk();
		countResumeCopy(count);
		expect(counted).toEqual(['copy.resume']);
	});
});
