<script lang="ts">
	import { FolderGit2 } from '@lucide/svelte';
	import { projectHref, promptHref, sessionHref } from '#lib/shared/links.ts';
	import LinkArrow from '#lib/ui/LinkArrow.svelte';
	import Panel from '#lib/ui/Panel.svelte';

	/** How a link to another level reads: accent text ending in "›", its target on hover. */

	const SAMPLES = [
		{ href: sessionHref('s1'), title: 'Session › Rust switch', label: 'Read the story' },
		{
			href: projectHref('p1', { anchor: 'prompt-7' }),
			title: 'Project › gitprompt',
			label: 'Show in project story'
		}
	];
</script>

<Panel title="Links">
	<div class="row">
		{#each SAMPLES as sample (sample.href)}
			<LinkArrow href={sample.href} title={sample.title}>{sample.label}</LinkArrow>
		{/each}
		<LinkArrow href={projectHref('p1')} title="Project › gitprompt"
			><FolderGit2 size={14} aria-hidden="true" />With an icon</LinkArrow
		>
	</div>
	<p class="muted">
		Hover shows the target. A plain <a href={promptHref(1)} title="Prompt">text link</a> opens a
		prompt without changing level; every link is built in <code>shared/links.ts</code>.
	</p>
</Panel>
