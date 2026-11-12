// The architecture drawn from the code (`just arch-report`): the rules of
// backend/crates/gitprompt-core/tests/architecture/rules.rs, the modularity score, and the full
// interactive page (layers, module matrix, coverage per crate when tests ran first).
import { join } from 'node:path';

import { checks, coverageCell, embed, kpis, pct, table, tone, toneBelow } from '../../lib/spec.mjs';

export default {
	title: 'Architecture',
	description: 'Crate layers and module dependencies drawn from the code, the architecture rules, and a modularity score.',
	async run(ctx) {
		await ctx.exec('just', ['arch-report'], { label: 'drawing the architecture (just arch-report)' });
		const report = await ctx.readJson(join(ctx.backend, 'target/architecture/report.json'));
		const findings = report.checks.reduce((total, check) => total + check.problems.length, 0);
		const part = (name) => report.score.parts.find((p) => p.name === name)?.value ?? null;
		const modules = report.crates.reduce((total, c) => total + c.modules.length, 0);
		const scoreTone = findings ? 'bad' : tone(report.score.total, 85, 70);
		return {
			headline: { value: report.score.total, label: 'modularity / 100', tone: scoreTone },
			summary: `${findings} rule findings · propagation cost ${pct(1 - part('Module coupling'))} over ${modules} modules in ${report.crates.length} crates`,
			sections: [
				kpis([
					{ label: 'Modularity score', value: report.score.total, tone: scoreTone, hint: '40% rules, 35% module coupling, 25% file size' },
					{ label: 'Rule findings', value: findings, tone: findings ? 'bad' : 'good' },
					{ label: 'Propagation cost', value: pct(1 - part('Module coupling')), tone: toneBelow(1 - part('Module coupling'), 0.2, 0.35), hint: 'share of module pairs a change can reach' },
					{ label: 'Code in files over 400 lines', value: pct(1 - part('File size')), tone: toneBelow(1 - part('File size'), 0.1, 0.25) },
					{ label: 'Crates', value: report.crates.length },
					{ label: 'Modules', value: modules }
				]),
				checks(
					'Rules (CI fails on any finding)',
					report.checks.map((check) => ({ label: check.rule, ok: check.problems.length === 0, detail: check.problems.length ? `${check.problems.join('\n')}\n\n${check.advice}` : '' }))
				),
				table(
					'Crates, lowest layer first',
					[
						{ key: 'name', label: 'Crate', mono: true },
						{ key: 'lines', label: 'Lines', align: 'right' },
						{ key: 'modules', label: 'Modules', align: 'right' },
						{ key: 'cost', label: 'Propagation cost', align: 'right' },
						{ key: 'coverage', label: 'Coverage', align: 'right' },
						{ key: 'uses', label: 'Uses', align: 'right' },
						{ key: 'used', label: 'Used by', align: 'right' }
					],
					report.crates.map((c) => ({
						name: { text: c.name, title: c.role },
						lines: c.lines,
						modules: c.modules.length,
						cost: { text: pct(c.propagation_cost), sort: c.propagation_cost, tone: toneBelow(c.propagation_cost, 0.25, 0.4) },
						coverage: c.coverage ? coverageCell(c.coverage.covered, c.coverage.count) : { text: '–', sort: -1 },
						uses: c.uses.length,
						used: c.used_by.length
					}))
				),
				embed('Interactive map: layers, module matrix, largest files', ctx.fromHub(join(ctx.backend, 'target/architecture/index.html')))
			]
		};
	}
};
