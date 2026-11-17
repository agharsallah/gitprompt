// "Connect a tool": the connectors' own command, to the page where each tool is set up.
import { goto } from '$app/navigation';
import { registerCommand } from '#lib/palette/commands.ts';
import { sourcesHref } from '#lib/shared/links.ts';

registerCommand({
	id: 'sources.connect',
	title: 'Connect a tool',
	keywords: ['connector', 'claude', 'cursor', 'codex', 'copilot', 'setup'],
	run: () => goto(sourcesHref())
});
