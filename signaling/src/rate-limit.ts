import { DurableObject } from "cloudflare:workers";

export type Window = { start: number; count: number };

/** Fixed-window counter. Every call counts, including denied ones. */
export function allow(state: Window | undefined, now: number, limit: number, windowMs: number): { ok: boolean; next: Window } {
	if (state === undefined || now - state.start >= windowMs) {
		const next = { start: now, count: 1 };
		return { ok: next.count <= limit, next };
	}
	const next = { start: state.start, count: state.count + 1 };
	return { ok: next.count <= limit, next };
}

/** One instance per bucket key (e.g. "viewer-ip:<ip>"). */
export class RateLimit extends DurableObject<Env> {
	hit(limit: number, windowMs: number): boolean {
		const state = this.ctx.storage.kv.get<Window>("window");
		const { ok, next } = allow(state, Date.now(), limit, windowMs);
		this.ctx.storage.kv.put("window", next);
		return ok;
	}
}
