<script lang="ts">
	import { FolderGit2, Folder, GitBranch } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import type { ProjectSummary } from '#lib/projects/summary.ts';
	import { KIND_INFO } from '#lib/insights/tools.ts';
	import { integer, money, plural, relativePath } from '#lib/shared/format.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import RelativeTime from '#lib/ui/RelativeTime.svelte';
	import SegmentedBar from '#lib/ui/charts/SegmentedBar.svelte';
	import Sparkline from '#lib/ui/charts/Sparkline.svelte';

	/** One project: how much you worked in it, when, on which branches, and what the agent did there. */
	let { summary, share }: { summary: ProjectSummary; /** of all prompts, 0..1 */ share: number } =
		$props();

	const p = $derived(summary);
	const BRANCHES = 3;
</script>

<article class="project" aria-labelledby="project-{p.key}">
	<div class="name">
		{#if p.git}<FolderGit2 size={18} aria-hidden="true" />{:else}<Folder
				size={18}
				aria-hidden="true"
			/>{/if}
		<div class="titles">
			<h2 id="project-{p.key}">
				<a href="{resolve('/prompts')}?repo={encodeURIComponent(p.key)}">{p.name}</a>
			</h2>
			<p class="path mono" title={p.key}>{relativePath(p.key)}</p>
		</div>
	</div>

	<div class="volume">
		<p class="prompts">
			<span class="n">{integer(summary.prompts)}</span>
			<span class="muted">{summary.prompts === 1 ? 'prompt' : 'prompts'}</span>
		</p>
		<div
			class="share"
			role="meter"
			aria-label="Share of your prompts"
			aria-valuemin={0}
			aria-valuemax={100}
			aria-valuenow={Math.round(share * 100)}
		>
			<span style:width="{share * 100}%"></span>
		</div>
	</div>

	<dl class="facts">
		<div>
			<dt>Sessions</dt>
			<dd>
				<a href="{resolve('/sessions')}?project={encodeURIComponent(p.key)}"
					>{integer(summary.sessions)}</a
				>
			</dd>
		</div>
		<div>
			<dt title="Files each turn wrote, summed over turns">Files written</dt>
			<dd>{integer(summary.filesWritten)}</dd>
		</div>
		{#if summary.cost}<div>
				<dt>Cost</dt>
				<dd>{money(summary.cost)}</dd>
			</div>{/if}
		<div>
			<dt>Last prompt</dt>
			<dd><RelativeTime iso={summary.lastAt} /></dd>
		</div>
	</dl>

	<div class="trend">
		{#if summary.daily.length > 1}
			<Sparkline values={summary.daily} width={120} height={32} label="Prompts per day" />
		{/if}
	</div>

	<div class="detail">
		{#if summary.kinds.length}
			<SegmentedBar
				label="Tool calls by kind in {p.name}"
				parts={summary.kinds.map((k) => ({
					key: k.kind,
					label: KIND_INFO[k.kind].label,
					value: k.count,
					color: KIND_INFO[k.kind].color
				}))}
				format={(v) => plural(v, 'call')}
			/>
		{/if}
		<div class="branches">
			{#each summary.branches.slice(0, BRANCHES) as branch (branch)}
				<span class="branch"><GitBranch size={12} aria-hidden="true" />{branch}</span>
			{/each}
			{#if summary.branches.length > BRANCHES}<span class="muted"
					>{summary.branches.length - BRANCHES} more</span
				>{/if}
			{#if summary.failed}<Badge tone="danger" size="sm"
					>{plural(summary.failed, 'failed turn')}</Badge
				>{/if}
		</div>
	</div>
</article>

<style>
	.project {
		display: grid;
		grid-template-columns: minmax(12rem, 1.4fr) minmax(7rem, 0.6fr) minmax(16rem, 1.4fr) 8rem;
		grid-template-areas:
			'name volume facts trend'
			'name detail detail detail';
		gap: var(--space-3) var(--space-6);
		align-items: center;
		padding: var(--space-4) var(--space-5);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}

	.name {
		grid-area: name;
		display: flex;
		align-items: flex-start;
		gap: var(--space-3);
		min-width: 0;
		align-self: start;
	}

	.name > :global(svg) {
		margin-top: 2px;
		color: var(--color-accent);
	}

	.titles {
		display: grid;
		gap: var(--space-0_5);
		min-width: 0;
	}

	h2 {
		font-size: var(--text-lg);
		font-stretch: var(--stretch-normal);
		letter-spacing: var(--tracking-normal);
		overflow-wrap: anywhere;
	}

	h2 a {
		color: var(--color-text);
		text-decoration: none;
	}

	h2 a:hover {
		color: var(--color-accent-text);
		text-decoration: underline;
	}

	.path {
		overflow: hidden;
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.volume {
		grid-area: volume;
		display: grid;
		gap: var(--space-1);
	}

	.prompts {
		display: flex;
		align-items: baseline;
		gap: var(--space-1);
		font-size: var(--text-sm);
	}

	.n {
		font-size: var(--text-2xl);
		font-weight: var(--weight-semibold);
		font-stretch: var(--stretch-heading);
	}

	.share {
		height: 4px;
		border-radius: var(--radius-full);
		background: color-mix(in srgb, var(--color-accent) 16%, transparent);
	}

	.share span {
		display: block;
		height: 100%;
		min-width: 2px;
		border-radius: var(--radius-full);
		background: var(--color-accent);
	}

	.facts {
		grid-area: facts;
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2) var(--space-5);
		margin: 0;
	}

	.facts div {
		display: grid;
	}

	dt {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
	}

	dd {
		margin: 0;
		font-weight: var(--weight-semibold);
	}

	.trend {
		grid-area: trend;
		justify-self: end;
	}

	.detail {
		grid-area: detail;
		display: grid;
		gap: var(--space-2);
	}

	.branches {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-3);
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
	}

	.branch {
		display: inline-flex;
		align-items: center;
		gap: 3px;
	}

	@media (max-width: 64rem) {
		.project {
			grid-template-columns: minmax(0, 1fr) auto;
			grid-template-areas:
				'name trend'
				'volume volume'
				'facts facts'
				'detail detail';
		}
	}
</style>
