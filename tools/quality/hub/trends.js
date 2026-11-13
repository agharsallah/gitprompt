// The Trends page: each report's main numbers (history.js, lib/history.mjs) over its runs, one small
// chart per number with its latest value and the change since the previous and the first run; and
// the sparkline on each overview card. Classic script; defines window.QT, which app.js uses.
'use strict';

(function () {
	const { h, rich } = window.Q;
	const SVG = 'http://www.w3.org/2000/svg';

	const svg = (tag, attrs, ...children) => {
		const node = document.createElementNS(SVG, tag);
		for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
		node.append(...children);
		return node;
	};

	/** A value in its unit, as the report pages show it. */
	function format(value, unit) {
		if (unit === 'share') return `${(value * 100).toFixed(1)}%`;
		if (unit === 'ms') return value >= 1000 ? `${(value / 1000).toFixed(2)} s` : `${value < 10 ? value.toFixed(1) : Math.round(value)} ms`;
		if (unit === 'bytes') return value >= 1048576 ? `${(value / 1048576).toFixed(1)} MB` : `${(value / 1024).toFixed(value < 10240 ? 1 : 0)} KB`;
		if (unit === 'rate') return `${Math.round(value).toLocaleString('en-US')}/s`;
		return Number.isInteger(value) ? value.toLocaleString('en-US') : value.toFixed(1);
	}

	/** A change in the metric's unit: points for a share, the unit otherwise. */
	function formatChange(change, unit) {
		const size = Math.abs(change);
		return unit === 'share' ? `${(size * 100).toFixed(1)} pt` : format(size, unit);
	}

	const day = (at) => new Date(at).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
	const when = (at) => new Date(at).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });

	/** The runs that measured `key`: [{ at, commit, value }], oldest first. */
	const pointsOf = (entry, key) => entry.runs.filter((run) => Number.isFinite(run.values[key])).map((run) => ({ at: run.at, commit: run.commit, value: run.values[key] }));

	/** Each point's place in the plot, 0 to 1 both ways (y from the top), runs evenly spaced. */
	function layout(points) {
		const values = points.map((p) => p.value);
		let low = Math.min(...values);
		let high = Math.max(...values);
		const pad = (high - low) * 0.12 || Math.abs(high) * 0.05 || 1;
		low -= pad;
		high += pad;
		return points.map((p, i) => ({ ...p, x: points.length === 1 ? 0.5 : i / (points.length - 1), y: 1 - (p.value - low) / (high - low) }));
	}

	/** The line through the points, in a 0..100 box stretched to the plot. */
	function line(placed, className) {
		const d = placed.map((p, i) => `${i ? 'L' : 'M'}${(p.x * 100).toFixed(2)},${(p.y * 100).toFixed(2)}`).join(' ');
		return svg('svg', { class: className, viewBox: '0 0 100 100', preserveAspectRatio: 'none', 'aria-hidden': 'true' }, svg('path', { d, 'vector-effect': 'non-scaling-stroke' }));
	}

	/** ▲ 1.2 pt since the previous run, toned by whether the change helps. */
	function change(metric, from, to, since) {
		const diff = to - from;
		if (diff === 0) return h('span', { class: 'muted' }, `= no change since ${since}`);
		const better = metric.better === 'lower' ? diff < 0 : metric.better === 'higher' ? diff > 0 : null;
		const tone = better === null ? 'muted' : better ? 't-good' : 't-bad';
		return h('span', { class: tone }, `${diff > 0 ? '▲' : '▼'} ${formatChange(diff, metric.unit)} since ${since}`);
	}

	/** The plot with its hover layer: a hairline and the point nearest the pointer, with a readout. */
	function plot(metric, placed) {
		const hair = h('div', { class: 'trend-hair', hidden: true });
		const dot = h('div', { class: 'trend-dot is-hover', hidden: true });
		const tip = h('div', { class: 'trend-tip', hidden: true });
		const last = placed.at(-1);
		const box = h(
			'div',
			{ class: 'trend-plot', tabindex: '0', role: 'img', 'aria-label': `${metric.label}: ${placed.length} runs, latest ${format(last.value, metric.unit)}. Arrow keys step through the runs.` },
			line(placed, 'trend-line'),
			h('div', { class: 'trend-dot', style: `left:${last.x * 100}%;top:${last.y * 100}%` }),
			hair,
			dot,
			tip
		);
		let at = null;
		const show = (index) => {
			at = Math.max(0, Math.min(placed.length - 1, index));
			const p = placed[at];
			hair.hidden = dot.hidden = tip.hidden = false;
			hair.style.left = `${p.x * 100}%`;
			dot.style.cssText = `left:${p.x * 100}%;top:${p.y * 100}%`;
			tip.replaceChildren(h('b', {}, format(p.value, metric.unit)), h('span', {}, when(p.at)), p.commit ? h('span', { class: 'mono' }, p.commit) : null);
			tip.style.left = `${p.x * 100}%`;
			tip.dataset.side = p.x > 0.6 ? 'left' : 'right';
		};
		const hide = () => {
			hair.hidden = dot.hidden = tip.hidden = true;
			at = null;
		};
		box.addEventListener('pointermove', (event) => {
			const rect = box.getBoundingClientRect();
			show(Math.round(((event.clientX - rect.left) / rect.width) * (placed.length - 1)));
		});
		box.addEventListener('pointerleave', hide);
		box.addEventListener('blur', hide);
		box.addEventListener('keydown', (event) => {
			if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
			event.preventDefault();
			show((at ?? placed.length - 1) + (event.key === 'ArrowLeft' ? -1 : 1));
		});
		return box;
	}

	/** One number: its latest value, how it moved, and its chart. */
	function tile(metric, entry) {
		const points = pointsOf(entry, metric.key);
		if (points.length === 0) return null;
		const last = points.at(-1);
		const values = points.map((p) => p.value);
		return h(
			'div',
			{ class: 'trend-tile' },
			h('div', { class: 'trend-label' }, metric.label),
			h('div', { class: 'trend-value' }, format(last.value, metric.unit)),
			h(
				'div',
				{ class: 'trend-changes' },
				points.length > 1 ? change(metric, points.at(-2).value, last.value, 'the previous run') : h('span', { class: 'muted' }, 'one run so far: the trend starts with the next'),
				points.length > 2 ? change(metric, points[0].value, last.value, day(points[0].at)) : null
			),
			plot(metric, layout(points)),
			h('div', { class: 'trend-foot muted' }, `${points.length} ${points.length === 1 ? 'run' : 'runs'} since ${day(points[0].at)} · range ${format(Math.min(...values), metric.unit)} to ${format(Math.max(...values), metric.unit)}`)
		);
	}

	/** Every run of a report as a table: the chart's numbers, readable without the chart. */
	function runsTable(entry) {
		const rows = [...entry.runs].reverse();
		return h(
			'details',
			{ class: 'trend-runs' },
			h('summary', {}, entry.runs.length === 1 ? 'The run as a table' : `All ${entry.runs.length} runs as a table`),
			h(
				'div',
				{ class: 'table-wrap' },
				h(
					'table',
					{},
					h('thead', {}, h('tr', {}, h('th', {}, 'Run'), h('th', {}, 'Commit'), entry.metrics.map((m) => h('th', { class: 'num' }, m.label)))),
					h('tbody', {}, rows.map((run) => h('tr', {}, h('td', {}, when(run.at)), h('td', { class: 'mono' }, run.commit || '–'), entry.metrics.map((m) => h('td', { class: 'num' }, Number.isFinite(run.values[m.key]) ? format(run.values[m.key], m.unit) : '–')))))
				)
			)
		);
	}

	function reportTrends(report, entry) {
		return h(
			'section',
			{ class: 'trend-report' },
			h('h3', {}, h('a', { href: `#${report.id}` }, report.title)),
			h('div', { class: 'trend-tiles' }, entry.metrics.map((m) => tile(m, entry))),
			runsTable(entry)
		);
	}

	/** The Trends page: per area, every report with recorded runs. */
	function page(status, history) {
		const recorded = (report) => history?.reports[report.id]?.runs.length > 0;
		const any = status.reports.some(recorded);
		return h(
			'div',
			{ class: 'page' },
			h('header', { class: 'page-head' }, h('div', {}, h('h1', {}, 'Trends'), h('p', { class: 'lead' }, rich('Each report\'s main numbers over its runs: every run that built the report adds a point (an up-to-date report adds none, its numbers did not change). Hover or use the arrow keys on a chart for a run\'s value, time and commit (`+`: with uncommitted changes).')))),
			any ? null : h('p', { class: 'callout c-neutral' }, rich('No run recorded yet. Every report adds a point each time it builds: `just quality` for the checks, `just quality measure` for the measurements.')),
			status.areas.map((area) => {
				const reports = status.reports.filter((r) => r.area === area.id);
				const shown = reports.filter(recorded);
				const missing = reports.filter((r) => !recorded(r));
				if (!any) return null;
				return h(
					'section',
					{ class: 'area' },
					h('div', { class: 'area-head' }, h('h2', {}, area.title)),
					shown.map((r) => reportTrends(r, history.reports[r.id])),
					missing.length ? h('p', { class: 'muted trend-missing' }, rich(`No run recorded yet: ${missing.map((r) => `\`${r.command}\``).join(', ')}`)) : null
				);
			})
		);
	}

	/** A card's sparkline: the report's first number over its last runs, once there are two. */
	function sparkline(entry) {
		const metric = entry?.metrics[0];
		const points = metric ? pointsOf(entry, metric.key).slice(-30) : [];
		if (points.length < 2) return null;
		return h('div', { class: 'card-spark', title: `${metric.label}, last ${points.length} runs: ${format(points[0].value, metric.unit)} → ${format(points.at(-1).value, metric.unit)}` }, line(layout(points), 'spark-line'));
	}

	window.QT = { page, sparkline, format };
})();
