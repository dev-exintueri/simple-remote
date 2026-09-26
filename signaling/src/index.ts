import { HostRoom } from "./host-room";
import { RateLimit } from "./rate-limit";

export { HostRoom, RateLimit };

const WINDOW_MS = 10 * 60 * 1000;
const VIEWER_PER_IP = 30;
const VIEWER_PER_ID = 60;
const HOST_PER_IP = 20;
const CLAIM_ATTEMPTS = 5;

const ID_MIN = 100_000_000;
const ID_SPAN = 900_000_000;
// Largest multiple of ID_SPAN below 2^32, for rejection sampling without modulo bias.
const ID_SAMPLE_LIMIT = Math.floor(0x1_0000_0000 / ID_SPAN) * ID_SPAN;

function randomId(): string {
	const buf = new Uint32Array(1);
	for (;;) {
		crypto.getRandomValues(buf);
		if (buf[0] < ID_SAMPLE_LIMIT) return String(ID_MIN + (buf[0] % ID_SPAN));
	}
}

async function hit(env: Env, bucket: string, limit: number): Promise<boolean> {
	const stub = env.RATE_LIMIT.get(env.RATE_LIMIT.idFromName(bucket));
	return stub.hit(limit, WINDOW_MS);
}

function status(code: number, text: string): Response {
	return new Response(text, { status: code });
}

function roomRequest(path: string, params: Record<string, string>): Request {
	const url = new URL(`https://host-room${path}`);
	for (const [k, v] of Object.entries(params)) url.searchParams.set(k, v);
	return new Request(url, { headers: { Upgrade: "websocket" } });
}

function room(env: Env, id: string) {
	return env.HOST_ROOM.get(env.HOST_ROOM.idFromName(id));
}

export default {
	async fetch(request, env) {
		const url = new URL(request.url);
		const hostNew = url.pathname === "/v1/host";
		const hostExisting = url.pathname.match(/^\/v1\/host\/([1-9]\d{8})$/);
		const viewer = url.pathname.match(/^\/v1\/viewer\/([1-9]\d{8})$/);
		if (request.method !== "GET" || (!hostNew && hostExisting === null && viewer === null)) {
			return status(404, "not found");
		}
		if (request.headers.get("Upgrade") !== "websocket") return status(426, "expected websocket");
		const ip = request.headers.get("CF-Connecting-IP");
		if (ip === null || ip === "") return status(400, "missing client ip");

		if (viewer !== null) {
			const kind = url.searchParams.get("kind") ?? "";
			if (kind !== "new" && kind !== "reconnect") return status(400, "bad kind");
			const id = viewer[1];
			const [ipOk, idOk] = await Promise.all([
				hit(env, `viewer-ip:${ip}`, VIEWER_PER_IP),
				hit(env, `viewer-id:${id}`, VIEWER_PER_ID),
			]);
			if (!ipOk || !idOk) return status(429, "too many requests");
			return room(env, id).fetch(roomRequest("/viewer", { id, kind }));
		}

		if (!(await hit(env, `host-ip:${ip}`, HOST_PER_IP))) return status(429, "too many requests");
		const rawKey = url.searchParams.get("key") ?? "";
		if (!/^[0-9a-fA-F]{64}$/.test(rawKey)) return status(400, "bad key");
		const key = rawKey.toLowerCase();

		if (hostExisting !== null) {
			const id = hostExisting[1];
			return room(env, id).fetch(roomRequest("/host", { id, key }));
		}
		for (let i = 0; i < CLAIM_ATTEMPTS; i++) {
			const id = randomId();
			const res = await room(env, id).fetch(roomRequest("/host", { id, key, claim: "1" }));
			if (res.status !== 409) return res;
		}
		return status(503, "no free id");
	},
} satisfies ExportedHandler<Env>;
