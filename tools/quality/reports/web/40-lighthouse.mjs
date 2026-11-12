// Lighthouse on the real dashboard: the fresh web build served by a debug daemon with demo data
// (tools/seed-demo.py, in a temporary folder), desktop preset, a few key pages. Scores per page,
// the core web vitals, and each page's full Lighthouse report.
import { join } from 'node:path';

import { demoDaemon } from '../../lib/shared.mjs';
import { embed, kpis, links, ms, note, table, tone, toneBelow } from '../../lib/spec.mjs';

const LIGHTHOUSE = 'lighthouse@12.8.2';
const PAGES = [
	{ path: '/', name: 'Overview' },
	{ path: '/prompts', name: 'Prompts' },
	{ path: '/sessions', name: 'Sessions' },
	{ path: '/sources', name: 'Sources' }
];
const CATEGORIES = [
	{ key: 'performance', label: 'Performance' },
	{ key: 'accessibility', label: 'Accessibility' },
	{ key: 'best-practices', label: 'Best practices' },
	{ key: 'seo', label: 'SEO' }
];
/** Lighthouse's own colours: 90 and up is good, 50 to 89 needs work. */
const scoreTone = (score) => tone(score, 90, 50);

export default {
	title: 'Lighthouse',
	description: 'Performance, accessibility, best practices and SEO of key pages, on the built dashboard with demo data.',
	needs: [
		{ command: 'pnpm', install: 'https://pnpm.io/installation' },
		{ command: 'cargo', install: 'https://rustup.rs' }
	],
	async run(ctx) {
		const { url } = await demoDaemon(ctx);
		const results = [];
		for (const page of PAGES) {
			const slug = page.path === '/' ? 'home' : page.path.slice(1).replaceAll('/', '-');
			const base = join(ctx.artifacts, slug);
			await ctx.exec(
				'pnpm',
				['dlx', LIGHTHOUSE, `${url}${page.path}`, '--quiet', '--preset=desktop', '--output=json', '--output=html', `--output-path=${base}`, `--only-categories=${CATEGORIES.map((c) => c.key).join(',')}`, '--chrome-flags=--headless=new --no-first-run --no-default-browser-check'],
				{ label: `Lighthouse ${page.path} (${results.length + 1}/${PAGES.length})` }
			);
			const report = await ctx.readJson(`${base}.report.json`);
			results.push({ page, html: `${base}.report.html`, report });
		}
		return render(ctx, results);
	}
};

const score = (report, key) => Math.round((report.categories[key]?.score ?? 0) * 100);
const metric = (report, id) => report.audits[id]?.numericValue ?? null;

function render(ctx, results) {
	const average = (key) => Math.round(results.reduce((t, r) => t + score(r.report, key), 0) / results.length);
	const perf = average('performance');
	return {
		headline: { value: perf, label: `performance (average of ${results.length} pages)`, tone: scoreTone(perf) },
		summary: CATEGORIES.map((c) => `${c.label} ${average(c.key)}`).join(' · '),
		sections: [
			kpis(CATEGORIES.map((c) => ({ label: c.label, value: average(c.key), tone: scoreTone(average(c.key)), hint: 'average over the pages' }))),
			note(`Desktop preset, ${LIGHTHOUSE}, against a local daemon with demo data: no network latency, so real installs score the same or better. Scores move a few points between runs.`),
			table(
				'Pages',
				[
					{ key: 'page', label: 'Page' },
					...CATEGORIES.map((c) => ({ key: c.key, label: c.label, align: 'right' })),
					{ key: 'lcp', label: 'LCP', align: 'right' },
					{ key: 'tbt', label: 'TBT', align: 'right' },
					{ key: 'cls', label: 'CLS', align: 'right' }
				],
				results.map(({ page, html, report }) => {
					const lcp = metric(report, 'largest-contentful-paint');
					const tbt = metric(report, 'total-blocking-time');
					const cls = metric(report, 'cumulative-layout-shift');
					return {
						page: { text: `${page.name} (${page.path})`, href: ctx.fromHub(html) },
						...Object.fromEntries(CATEGORIES.map((c) => [c.key, { text: score(report, c.key), tone: scoreTone(score(report, c.key)), sort: score(report, c.key) }])),
						lcp: { text: lcp === null ? '–' : ms(lcp), sort: lcp, tone: toneBelow(lcp, 2500, 4000) },
						tbt: { text: tbt === null ? '–' : ms(tbt), sort: tbt, tone: toneBelow(tbt, 200, 600) },
						cls: { text: cls === null ? '–' : cls.toFixed(3), sort: cls, tone: toneBelow(cls, 0.1, 0.25) }
					};
				}),
				'LCP: largest contentful paint (good ≤ 2.5 s). TBT: total blocking time (≤ 200 ms). CLS: cumulative layout shift (≤ 0.1).'
			),
			links(
				'Full Lighthouse reports',
				results.map(({ page, html }) => ({ label: `${page.name} (${page.path})`, href: ctx.fromHub(html) }))
			),
			embed(`Full report: ${results[0].page.name}`, ctx.fromHub(results[0].html))
		]
	};
}
