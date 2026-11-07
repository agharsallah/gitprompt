// First run: the steps from nothing to a captured prompt, from the connectors' state. Each connect
// step links to /sources?connect=<id>, which opens that connector's consent review directly, so the
// whole path is one click here plus one confirmation there. Pure, tested in setup.test.ts.
import type { Source } from '#lib/api/types.ts';
import { layerName, tools } from '#lib/sources/state.ts';

export type SetupStep = {
	id: string;
	title: string;
	body: string;
	done: boolean;
	optional: boolean;
	/** The connector to connect; none for the last step, which happens in the tool. */
	connect?: string;
	action?: string;
};

/** The tool to start with: the first one installed on this machine, else the first one known. */
const firstTool = (sources: Source[]) => {
	const all = tools(sources);
	return all.find((t) => t.layers[0].detected) ?? all[0];
};

export function setupSteps(sources: Source[]): SetupStep[] {
	const tool = firstTool(sources);
	if (!tool) return [];
	const [main, ...extra] = tool.layers;
	return [
		{
			id: main.id,
			title: `Connect ${tool.name}`,
			body: main.detected
				? 'Review the exact change to its settings file, then confirm. A backup is kept.'
				: `Install ${tool.name} first; GitPrompt can’t find it on this machine.`,
			done: main.connected,
			optional: false,
			connect: main.id,
			action: 'Connect'
		},
		...extra.map((layer) => ({
			id: layer.id,
			title: `Add ${layerName(layer).toLowerCase()}`,
			body: layer.summary || layer.name,
			done: layer.connected,
			optional: true,
			connect: layer.id,
			action: `Add ${layerName(layer).toLowerCase()}`
		})),
		{
			id: 'first-prompt',
			title: 'Send a prompt',
			body: `Start a new ${tool.name} session and type anything. It appears here as soon as you send it.`,
			done: sources.some((s) => s.total > 0),
			optional: false
		}
	];
}

/** Where a connect step goes. */
export const connectQuery = (id: string) => `?connect=${encodeURIComponent(id)}`;
