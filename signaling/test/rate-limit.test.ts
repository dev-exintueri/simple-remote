import { describe, expect, it } from "vitest";
import { allow, type Window } from "../src/rate-limit";

const WINDOW_MS = 10 * 60 * 1000;

describe("allow", () => {
	it("allows up to the limit, rejects limit+1, allows again after the window", () => {
		let state: Window | undefined;
		const t0 = 1_000_000;
		for (let i = 0; i < 3; i++) {
			const r = allow(state, t0 + i, 3, WINDOW_MS);
			expect(r.ok).toBe(true);
			state = r.next;
		}
		const denied = allow(state, t0 + 10, 3, WINDOW_MS);
		expect(denied.ok).toBe(false);
		state = denied.next;

		const stillDenied = allow(state, t0 + WINDOW_MS - 1, 3, WINDOW_MS);
		expect(stillDenied.ok).toBe(false);
		state = stillDenied.next;

		const fresh = allow(state, t0 + WINDOW_MS, 3, WINDOW_MS);
		expect(fresh.ok).toBe(true);
		expect(fresh.next).toEqual({ start: t0 + WINDOW_MS, count: 1 });
	});

	it("starts a window on first use", () => {
		expect(allow(undefined, 5, 1, WINDOW_MS)).toEqual({ ok: true, next: { start: 5, count: 1 } });
	});
});
