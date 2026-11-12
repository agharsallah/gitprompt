/**
 * A fetch for tests: answers each request from `reply` (by URL) and records what was asked, so a
 * test checks both the URL, method, headers and body a client sends and how it reads the answer.
 */
export type Call = { url: string; method: string; headers: Record<string, string>; body: unknown };
export type Reply = { status?: number; statusText?: string; body?: unknown; raw?: string };

export function fakeFetch(reply: (url: string) => Reply = () => ({ body: {} })) {
	const calls: Call[] = [];
	const fetcher = (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
		const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
		const headers = Object.fromEntries(new Headers(init?.headers).entries());
		const body: unknown = typeof init?.body === 'string' ? JSON.parse(init.body) : undefined;
		calls.push({ url, method: init?.method ?? 'GET', headers, body });
		const answer = reply(url);
		const text = answer.raw ?? JSON.stringify(answer.body ?? null);
		const response = new Response(text, {
			status: answer.status ?? 200,
			statusText: answer.statusText ?? ''
		});
		return Promise.resolve(response);
	};
	const typed: typeof fetch = fetcher;
	return { fetch: typed, calls };
}
