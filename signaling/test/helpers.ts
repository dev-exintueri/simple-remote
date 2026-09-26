import { exports } from "cloudflare:workers";
import { expect } from "vitest";

const AUTH_PREFIX = new TextEncoder().encode("simple-remote signaling auth v1");

// Every test uses its own client IP so the per-IP rate limits never leak between tests
// (storage is shared across test files under --no-isolate).
let ipCounter = 0;
export function freshIp(): string {
	ipCounter += 1;
	const r = crypto.getRandomValues(new Uint8Array(2));
	return `10.${r[0]}.${r[1]}.${ipCounter % 250}`;
}

export function toHex(bytes: Uint8Array): string {
	return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

function fromHex(hex: string): Uint8Array {
	const out = new Uint8Array(hex.length / 2);
	for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
	return out;
}

export interface Keypair {
	pub: string;
	priv: CryptoKey;
}

export async function newKey(): Promise<Keypair> {
	const pair = (await crypto.subtle.generateKey({ name: "Ed25519" }, true, ["sign", "verify"])) as CryptoKeyPair;
	const raw = (await crypto.subtle.exportKey("raw", pair.publicKey)) as ArrayBuffer;
	return { pub: toHex(new Uint8Array(raw)), priv: pair.privateKey };
}

export async function sign(priv: CryptoKey, nonceHex: string, id: string): Promise<string> {
	const nonce = fromHex(nonceHex);
	const idBytes = new TextEncoder().encode(id);
	const msg = new Uint8Array(AUTH_PREFIX.length + nonce.length + idBytes.length);
	msg.set(AUTH_PREFIX, 0);
	msg.set(nonce, AUTH_PREFIX.length);
	msg.set(idBytes, AUTH_PREFIX.length + nonce.length);
	const sig = await crypto.subtle.sign({ name: "Ed25519" }, priv, msg);
	return toHex(new Uint8Array(sig));
}

export async function request(path: string, ip: string | null, upgrade = true): Promise<Response> {
	const headers: Record<string, string> = {};
	if (upgrade) headers.Upgrade = "websocket";
	if (ip !== null) headers["CF-Connecting-IP"] = ip;
	return exports.default.fetch(new Request(`https://example.com${path}`, { headers }));
}

/** A client socket that buffers every message from the moment it is accepted. */
export class Peer {
	readonly ws: WebSocket;
	readonly closed: Promise<number>;
	private readonly queue: unknown[] = [];
	private readonly waiters: ((m: unknown) => void)[] = [];

	constructor(ws: WebSocket) {
		this.ws = ws;
		ws.accept();
		ws.addEventListener("message", (e) => {
			const text = typeof e.data === "string" ? e.data : new TextDecoder().decode(e.data as ArrayBuffer);
			const msg: unknown = JSON.parse(text);
			const waiter = this.waiters.shift();
			if (waiter) waiter(msg);
			else this.queue.push(msg);
		});
		this.closed = new Promise((resolve) => {
			ws.addEventListener("close", (e) => resolve(e.code));
		});
	}

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	next(timeoutMs = 5_000): Promise<any> {
		if (this.queue.length > 0) return Promise.resolve(this.queue.shift());
		return new Promise((resolve, reject) => {
			const timer = setTimeout(() => reject(new Error("timeout waiting for message")), timeoutMs);
			this.waiters.push((m) => {
				clearTimeout(timer);
				resolve(m);
			});
		});
	}

	/** Resolves true if no message arrives within `ms`. */
	async silentFor(ms: number): Promise<boolean> {
		if (this.queue.length > 0) return false;
		await new Promise((r) => setTimeout(r, ms));
		return this.queue.length === 0;
	}

	send(msg: unknown): void {
		this.ws.send(typeof msg === "string" ? msg : JSON.stringify(msg));
	}

	closeCode(timeoutMs = 5_000): Promise<number> {
		return Promise.race([
			this.closed,
			new Promise<number>((_, reject) => setTimeout(() => reject(new Error("timeout waiting for close")), timeoutMs)),
		]);
	}
}

export async function connect(path: string, ip: string): Promise<Peer> {
	const res = await request(path, ip);
	expect(res.status).toBe(101);
	const ws = res.webSocket;
	if (!ws) throw new Error("no webSocket on response");
	return new Peer(ws);
}

/** Tells an authed host to switch mode (no reply is expected). */
export function setMode(peer: Peer, mode: "new" | "reconnect"): void {
	peer.send({ t: "mode", mode });
}

/** Connects a host (new registration when `id` is omitted), signs the challenge and waits for `registered`. */
export async function registerHost(key: Keypair, ip: string, id?: string): Promise<{ peer: Peer; id: string }> {
	const path = id === undefined ? `/v1/host?key=${key.pub}` : `/v1/host/${id}?key=${key.pub}`;
	const peer = await connect(path, ip);
	const challenge = await peer.next();
	expect(challenge.t).toBe("challenge");
	peer.send({ t: "auth", sig: await sign(key.priv, challenge.nonce, challenge.id) });
	const registered = await peer.next();
	expect(registered).toEqual({ t: "registered", id: challenge.id });
	return { peer, id: registered.id };
}
