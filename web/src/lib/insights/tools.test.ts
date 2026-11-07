import { describe, expect, it } from 'vitest';
import {
	byKind,
	KIND_INFO,
	mcpParts,
	TOOL_KINDS,
	asKind,
	kindOf,
	toolLabel
} from '#lib/insights/tools.ts';

describe('kindOf', () => {
	it('takes the kind the daemon gave the tool', () => {
		expect(kindOf('Bash', { Bash: 'shell' })).toBe('shell');
		expect(kindOf('mcp__x__y', { mcp__x__y: 'mcp' })).toBe('mcp');
	});

	it('draws unknown tools and kinds as other', () => {
		expect(kindOf('Bash', undefined)).toBe('other');
		expect(kindOf('Bash', { Bash: 'teleport' })).toBe('other');
		expect(asKind(null)).toBe('other');
	});
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
	it('orders kinds, drops empty ones and folds unknown ones into other', () => {
		expect(byKind({ shell: 3, read: 3, mcp: 4, teleport: 1, other: 1 })).toEqual([
			{ kind: 'read', count: 3 },
			{ kind: 'shell', count: 3 },
			{ kind: 'mcp', count: 4 },
			{ kind: 'other', count: 2 }
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
