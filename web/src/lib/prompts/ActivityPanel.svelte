<script lang="ts">
	import { FileCode, FilePen, Globe } from '@lucide/svelte';
	import type { Prompt } from '#lib/api/types.ts';
	import { byKind, KIND_INFO, kindOf, toolLabel } from '#lib/insights/tools.ts';
	import { count, integer, plural, relativePath } from '#lib/shared/format.ts';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import SegmentedBar from '#lib/ui/charts/SegmentedBar.svelte';
	import CodeBlock from '#lib/ui/CodeBlock.svelte';
	import Disclosure from '#lib/ui/Disclosure.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';

	/** What the agent did for this prompt: which tools, which files, which commands, in order. */
	let { prompt }: { prompt: Prompt } = $props();

	const s = $derived(prompt.summary);
	const root = $derived(prompt.context.git_root ?? prompt.fields.cwd);
	const kinds = $derived(byKind(s.tool_kinds));
	const tools = $derived(
		Object.entries(s.tool_calls ?? {})
			.sort((a, b) => b[1] - a[1])
			.map(([name, value]) => {
				const info = KIND_INFO[kindOf(name, s.tool_kind_of)];
				return {
					key: name,
					label: toolLabel(name),
					value,
					color: info.color,
					icon: info.icon,
					mono: name.startsWith('mcp__')
				};
			})
	);
	const COMMANDS_SHOWN = 6;
	let allCommands = $state(false);
	const commands = $derived(s.bash_commands ?? []);

	const extras = $derived(
		[
			s.subagents &&
				count(s.subagents) && [
					'Subagents',
					Object.entries(s.subagents).map(([k, v]) => `${k || 'general'} × ${v}`)
				],
			s.tasks?.length && ['Tasks created', s.tasks],
			s.notifications &&
				count(s.notifications) && [
					'Notifications',
					Object.entries(s.notifications).map(([k, v]) => `${k} × ${v}`)
				],
			s.permission_denied?.length && ['Permission denied', s.permission_denied],
			s.tool_rejections && ['Tool calls you rejected', [String(s.tool_rejections)]],
			s.compactions && ['Context compactions', [String(s.compactions)]],
			s.model_switches?.length && ['Model switches', s.model_switches]
		].filter((x): x is [string, string[]] => Array.isArray(x))
	);
</script>

{#if !tools.length && !extras.length}
	<EmptyState compact title="No tools were used">
		The agent answered without reading, editing or running anything.
	</EmptyState>
{:else}
	<div class="activity">
		{#if tools.length}
			<section class="block">
				<h3>Tool calls <span class="muted num">{integer(count(s.tool_calls))}</span></h3>
				<SegmentedBar
					variant="tall"
					label="Tool calls by kind"
					parts={kinds.map((k) => ({
						key: k.kind,
						label: KIND_INFO[k.kind].label,
						value: k.count,
						color: KIND_INFO[k.kind].color
					}))}
					format={(v) => plural(v, 'call')}
				/>
				<BarList items={tools} label="Tool calls by tool" max={10} otherLabel="Other tools" />
				{#if s.tool_failures || s.tool_time_s}
					<p class="note">
						{#if s.tool_failures}<span class="danger-text"
								>{plural(s.tool_failures, 'call')} failed.</span
							>{/if}
						{#if s.tool_time_s}Tools ran for {s.tool_time_s}s in total.{/if}
					</p>
				{/if}
			</section>
		{/if}

		{#if s.files_written?.length}
			<section class="block">
				<h3>Files changed <span class="muted num">{s.files_written.length}</span></h3>
				<ul class="files">
					{#each s.files_written as file (file)}
						<li>
							<FilePen size={14} aria-hidden="true" /><span class="mono" title={file}
								>{relativePath(file, root)}</span
							>
						</li>
					{/each}
				</ul>
			</section>
		{/if}

		{#if s.files_read?.length}
			<Disclosure summary="Files read">
				{#snippet meta()}<span class="num">{s.files_read!.length}</span>{/snippet}
				<ul class="files">
					{#each s.files_read as file (file)}
						<li>
							<FileCode size={14} aria-hidden="true" /><span class="mono" title={file}
								>{relativePath(file, root)}</span
							>
						</li>
					{/each}
				</ul>
			</Disclosure>
		{/if}

		{#if commands.length}
			<section class="block">
				<h3>Commands, in order <span class="muted num">{commands.length}</span></h3>
				<ol class="commands">
					{#each allCommands ? commands : commands.slice(0, COMMANDS_SHOWN) as command, i (i)}
						<li>
							<span class="step num" aria-hidden="true">{i + 1}</span>
							<CodeBlock code={command} maxHeight="10rem" />
						</li>
					{/each}
				</ol>
				{#if commands.length > COMMANDS_SHOWN}
					<button class="more" type="button" onclick={() => (allCommands = !allCommands)}>
						{allCommands ? 'Show fewer' : `Show all ${commands.length} commands`}
					</button>
				{/if}
			</section>
		{/if}

		{#if s.web?.length}
			<section class="block">
				<h3>Web</h3>
				<ul class="files">
					{#each s.web as item, i (i)}
						<li>
							<Globe size={14} aria-hidden="true" />
							{#if /^https?:\/\//.test(item)}
								<a href={item} rel="noreferrer noopener" target="_blank" class="mono">{item}</a>
							{:else}
								<span>Searched: {item}</span>
							{/if}
						</li>
					{/each}
				</ul>
			</section>
		{/if}

		{#if extras.length}
			<section class="block">
				<h3>Also in this turn</h3>
				<dl class="extras">
					{#each extras as [label, values] (label)}
						<dt>{label}</dt>
						<dd>{values.join(', ')}</dd>
					{/each}
				</dl>
			</section>
		{/if}
	</div>
{/if}

<style>
	.activity {
		display: grid;
		gap: var(--space-6);
	}

	.block {
		display: grid;
		gap: var(--space-3);
	}

	h3 {
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
		font-size: var(--text-md);
	}

	h3 .muted {
		font-size: var(--text-sm);
		font-weight: var(--weight-regular);
	}

	.note {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.danger-text {
		color: var(--color-danger-text);
	}

	.files {
		display: grid;
		gap: var(--space-1);
		font-size: var(--text-sm);
	}

	.files li {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
	}

	.files :global(svg) {
		color: var(--color-text-muted);
	}

	.files .mono,
	.files a {
		min-width: 0;
		overflow-wrap: anywhere;
		font-size: var(--text-xs);
	}

	.commands {
		display: grid;
		gap: var(--space-2);
	}

	.commands li {
		display: grid;
		grid-template-columns: 1.5rem minmax(0, 1fr);
		align-items: start;
		gap: var(--space-2);
	}

	.step {
		padding-top: var(--space-2);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		text-align: right;
	}

	.more {
		justify-self: start;
		color: var(--color-accent-text);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}

	.more:hover {
		text-decoration: underline;
	}

	.extras {
		display: grid;
		grid-template-columns: max-content minmax(0, 1fr);
		gap: var(--space-1) var(--space-4);
		font-size: var(--text-sm);
	}

	.extras dt {
		color: var(--color-text-secondary);
	}

	.extras dd {
		margin: 0;
		overflow-wrap: anywhere;
	}
</style>
