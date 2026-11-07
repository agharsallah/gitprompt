import { describe, expect, it } from 'vitest';
import {
	byKind,
	KIND_INFO,
	mcpParts,
	TOOL_KINDS,
	toolKind,
	toolLabel
} from '#lib/insights/tools.ts';

describe('toolKind', () => {
	it.each([
		['Read', 'read'],
		['Grep', 'read'],
		['Edit', 'write'],
		['NotebookEdit', 'write'],
		['Bash', 'shell'],
		['WebFetch', 'web'],
		['mcp__claude_ai_Figma__get_screenshot', 'mcp'],
		['Agent', 'agent'],
		['TodoWrite', 'other']
	])('%s is %s', (name, kind) => expect(toolKind(name)).toBe(kind));
});

describe('MCP names', () => {
	it('splits server and tool', () => {
		expect(mcpParts('mcp__chrome-devtools__take_screenshot')).toEqual({
			server: 'chrome-devtools',
			tool: 'take_screenshot'
		});
	});

	it('labels MCP tools by their tool name, and built-ins as they are', () => {
		expect(toolLabel('mcp__chrome-devtools__click')).toBe('click');
		expect(toolLabel('Bash')).toBe('Bash');
	});

	it('is not fooled by a name without a server', () => expect(mcpParts('Bash')).toBeNull());
});

describe('byKind', () => {
	it('folds tools into kinds, in kind order, without empty kinds', () => {
		expect(byKind({ Bash: 3, Read: 2, Grep: 1, mcp__x__y: 4 })).toEqual([
			{ kind: 'read', count: 3 },
			{ kind: 'shell', count: 3 },
			{ kind: 'mcp', count: 4 }
		]);
	});

	it('is empty without tool calls', () => expect(byKind(undefined)).toEqual([]));
});

describe('kind colors', () => {
	it('gives each kind its own chart slot, in palette order, and "other" the neutral', () => {
		const colors = TOOL_KINDS.map((k) => KIND_INFO[k].color);
		expect(new Set(colors).size).toBe(colors.length);
		expect(colors.slice(0, 6)).toEqual([1, 2, 3, 4, 5, 6].map((n) => `var(--chart-${n})`));
		expect(KIND_INFO.other.color).toBe('var(--chart-other)');
	});
});
