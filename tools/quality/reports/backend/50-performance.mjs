// The load benchmark with its budgets, as CI checks it (`just perf`): ingest throughput, every
// dashboard read alone and under concurrent ingest with a slow git, the database writer, peak memory.
import { bars, checks, kpis, mb, ms, plural, table, toneBelow } from '../../lib/spec.mjs';

/** The default budget of the benchmark (--max-read-p95-ms). */
const READ_BUDGET_MS = 300;

export default {
	title: 'Performance',
	description: 'Load benchmark in release mode: ingest throughput, read latencies alone and under load, writer contention, memory.',
	async run(ctx) {
		const result = await ctx.exec('cargo', ['run', '--release', '-q', '-p', 'gitprompt-server', '--example', 'load', '--', '--check', '--json'], {
			cwd: ctx.backend,
			label: 'load benchmark (release build, then about a minute of load)',
			allowFailure: true
		});
		const start = result.stdout.indexOf('{');
		if (start < 0) throw new Error(`the benchmark printed no results (exit ${result.code}); see the log`);
		return page(JSON.parse(result.stdout.slice(start)));
	}
};

const readColumns = [
	{ key: 'name', label: 'Read' },
	{ key: 'path', label: 'Path', mono: true },
	{ key: 'requests', label: 'Requests', align: 'right' },
	{ key: 'p50', label: 'p50', align: 'right' },
	{ key: 'p95', label: 'p95', align: 'right' },
	{ key: 'max', label: 'Max', align: 'right' },
	{ key: 'size', label: 'KB', align: 'right' }
];

function readRow(read) {
	return {
		name: read.name,
		path: read.path,
		requests: read.requests,
		p50: { text: ms(read.p50_ms), sort: read.p50_ms },
		p95: { text: ms(read.p95_ms), sort: read.p95_ms, tone: toneBelow(read.p95_ms, READ_BUDGET_MS / 2, READ_BUDGET_MS) },
		max: { text: ms(read.max_ms), sort: read.max_ms },
		size: Math.round(read.bytes / 1024)
	};
}

function page(r) {
	const load = r.concurrent;
	const reads = load ? load.reads : r.reads;
	const slowest = reads.reduce((worst, read) => (read.p95_ms > worst.p95_ms ? read : worst), reads[0]);
	const over = r.over_budget || [];
	const headlineTone = over.length ? 'bad' : toneBelow(slowest.p95_ms, READ_BUDGET_MS / 2, READ_BUDGET_MS);
	const sections = [
		kpis([
			{ label: 'Ingest', value: `${Math.round(r.ingest.events_per_s).toLocaleString('en-US')}/s`, hint: `${r.ingest.ms_per_event.toFixed(2)} ms per event, ${plural(r.ingest.events, 'event')}` },
			{ label: 'Index build', value: ms(r.ingest.index_ms) },
			{ label: 'Database', value: mb(r.db_bytes) },
			{ label: 'Slowest p95 alone', value: ms(Math.max(...r.reads.map((x) => x.p95_ms))) },
			...(load
				? [
						{ label: 'Slowest p95 under load', value: ms(slowest.p95_ms), tone: headlineTone },
						{ label: 'Peak memory', value: load.peaks.rss_bytes ? mb(load.peaks.rss_bytes) : '–' },
						{ label: 'Max CPU', value: load.peaks.cpu_percent ? `${Math.round(load.peaks.cpu_percent)}%` : '–' },
						{ label: 'Writer held at most', value: `${load.writer.max_hold_ms} ms`, tone: toneBelow(load.writer.max_hold_ms, 250, 500) }
					]
				: [])
		]),
		checks(
			'Budgets (CI runs `just perf` and fails over any)',
			over.length ? over.map((text) => ({ label: text, ok: false })) : [{ label: `every read under ${READ_BUDGET_MS} ms at p95, writer and memory within budget`, ok: true }]
		),
		bars(
			`p95 ${load ? 'under load' : 'alone'}, against the ${READ_BUDGET_MS} ms budget`,
			[...reads].sort((a, b) => b.p95_ms - a.p95_ms).map((read) => ({ label: read.name, value: Math.min(1, read.p95_ms / READ_BUDGET_MS), text: ms(read.p95_ms), tone: toneBelow(read.p95_ms, READ_BUDGET_MS / 2, READ_BUDGET_MS) })),
			'A full bar is the budget.'
		)
	];
	if (load) {
		sections.push(
			table(`Reads under load (${load.seconds} s of concurrent ingest, git answering in ${load.git_ms} ms)`, readColumns, load.reads.map(readRow), `${load.ingested} payloads ingested meanwhile; the writer was entered ${load.writer.entries} times, waited over budget ${load.writer.slow_waits} times (max ${load.writer.max_wait_ms} ms), held over budget ${load.writer.slow_holds} times.`)
		);
	}
	sections.push(table(`Reads alone (${r.runs} timed runs each after one cold run)`, readColumns, r.reads.map(readRow), `${r.prompts} prompts × ${r.events_per_prompt} events each, through the axum router in-process.`));
	return {
		headline: { value: ms(slowest.p95_ms), label: `slowest read p95${load ? ' under load' : ''}`, tone: headlineTone },
		summary: `${over.length ? `${plural(over.length, 'budget')} exceeded` : 'every budget met'} · ${Math.round(r.ingest.events_per_s)} events/s ingested${load?.peaks.rss_bytes ? ` · peak ${mb(load.peaks.rss_bytes)}` : ''}`,
		sections
	};
}
