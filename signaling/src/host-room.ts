import { DurableObject } from "cloudflare:workers";
import { verifyEd25519 } from "./ed25519";
import { fromHex, toHex } from "./hex";

const AUTH_PREFIX = new TextEncoder().encode("simple-remote signaling auth v1");
const MAX_MESSAGE_BYTES = 65_536;

const CLOSE_PROTOCOL = 4000;
const CLOSE_KICKED = 4001;
const CLOSE_HOST_LEFT = 4002;
const CLOSE_AUTH = 4003;
const CLOSE_REPLACED = 4010;
const CLOSE_TOO_BIG = 4013;

type HostAttachment = { authed: boolean; nonce: string; key: string; id: string };
type ViewerAttachment = { closedByServer: boolean };

/**
 * One instance per 9-digit ID. The Worker calls it with an internal URL:
 * `/host?id=&key=[&claim=1]` or `/viewer?id=`.
 */
export class HostRoom extends DurableObject<Env> {
	fetch(request: Request): Response {
		if (request.headers.get("Upgrade") !== "websocket") {
			return new Response("expected websocket", { status: 426 });
		}
		const url = new URL(request.url);
		const id = url.searchParams.get("id") ?? "";
		if (url.pathname === "/host") {
			return this.acceptHost(id, url.searchParams.get("key") ?? "", url.searchParams.get("claim") === "1");
		}
		if (url.pathname === "/viewer") return this.acceptViewer();
		return new Response("not found", { status: 404 });
	}

	private acceptHost(id: string, key: string, claim: boolean): Response {
		const owner = this.ctx.storage.kv.get<string>("owner");
		if (owner === undefined && !claim) return new Response("unknown id", { status: 404 });
		if (owner !== undefined && owner !== key) {
			return new Response(claim ? "id taken" : "key mismatch", { status: claim ? 409 : 403 });
		}
		const { 0: client, 1: server } = new WebSocketPair();
		this.ctx.acceptWebSocket(server, ["host"]);
		const nonce = toHex(crypto.getRandomValues(new Uint8Array(32)));
		const attachment: HostAttachment = { authed: false, nonce, key, id };
		server.serializeAttachment(attachment);
		server.send(JSON.stringify({ t: "challenge", nonce, id }));
		return new Response(null, { status: 101, webSocket: client });
	}

	private acceptViewer(): Response {
		const host = this.authedHost();
		if (host === undefined) return new Response("host offline", { status: 404 });
		if (this.openViewer() !== undefined) return new Response("viewer already connected", { status: 409 });
		const { 0: client, 1: server } = new WebSocketPair();
		this.ctx.acceptWebSocket(server, ["viewer"]);
		const attachment: ViewerAttachment = { closedByServer: false };
		server.serializeAttachment(attachment);
		server.send(JSON.stringify({ t: "joined" }));
		host.send(JSON.stringify({ t: "viewer_joined" }));
		return new Response(null, { status: 101, webSocket: client });
	}

	async webSocketMessage(ws: WebSocket, message: string | ArrayBuffer): Promise<void> {
		const size = typeof message === "string" ? new TextEncoder().encode(message).length : message.byteLength;
		if (size > MAX_MESSAGE_BYTES) return this.closeFromServer(ws, CLOSE_TOO_BIG, "message too large");
		const msg = parse(message);
		if (msg === null) return this.closeFromServer(ws, CLOSE_PROTOCOL, "bad message");

		if (this.ctx.getTags(ws).includes("host")) return this.onHostMessage(ws, msg);

		if (msg.t === "relay" && typeof msg.data === "string") {
			this.authedHost()?.send(JSON.stringify({ t: "relay", data: msg.data }));
			return;
		}
		this.closeFromServer(ws, CLOSE_PROTOCOL, "bad message");
	}

	private async onHostMessage(ws: WebSocket, msg: { t: unknown; [k: string]: unknown }): Promise<void> {
		const att = ws.deserializeAttachment() as HostAttachment;
		if (!att.authed) {
			if (msg.t !== "auth" || typeof msg.sig !== "string") return this.closeFromServer(ws, CLOSE_AUTH, "auth required");
			return this.authenticate(ws, att, msg.sig);
		}
		if (msg.t === "relay" && typeof msg.data === "string") {
			this.openViewer()?.send(JSON.stringify({ t: "relay", data: msg.data }));
			return;
		}
		if (msg.t === "kick") {
			const viewer = this.openViewer();
			if (viewer !== undefined) this.closeFromServer(viewer, CLOSE_KICKED, "kicked");
			return;
		}
		this.closeFromServer(ws, CLOSE_PROTOCOL, "bad message");
	}

	private async authenticate(ws: WebSocket, att: HostAttachment, sig: string): Promise<void> {
		const nonce = fromHex(att.nonce);
		if (nonce === null) return this.closeFromServer(ws, CLOSE_AUTH, "auth failed");
		const idBytes = new TextEncoder().encode(att.id);
		const signed = new Uint8Array(AUTH_PREFIX.length + nonce.length + idBytes.length);
		signed.set(AUTH_PREFIX, 0);
		signed.set(nonce, AUTH_PREFIX.length);
		signed.set(idBytes, AUTH_PREFIX.length + nonce.length);
		if (!(await verifyEd25519(att.key, sig, signed))) return this.closeFromServer(ws, CLOSE_AUTH, "auth failed");

		// Two concurrent claims of a fresh ID may both reach this point; the first one binds it.
		const owner = this.ctx.storage.kv.get<string>("owner");
		if (owner !== undefined && owner !== att.key) return this.closeFromServer(ws, CLOSE_AUTH, "auth failed");
		if (owner === undefined) this.ctx.storage.kv.put("owner", att.key);

		const previous = this.authedHost();
		if (previous !== undefined && previous !== ws) {
			this.hostGone(previous);
			this.closeFromServer(previous, CLOSE_REPLACED, "replaced");
		}
		ws.serializeAttachment({ ...att, authed: true } satisfies HostAttachment);
		ws.send(JSON.stringify({ t: "registered", id: att.id }));
	}

	webSocketClose(ws: WebSocket, code: number, reason: string): void {
		this.onGone(ws);
		// The test runtime does not auto-reply to a client close frame; without this reply the
		// client stays in CLOSING. 1005/1006 are reserved and cannot be sent, so answer with 1000.
		// The socket may already be closed when the runtime did reply, hence the catch.
		try {
			ws.close(code === 1005 || code === 1006 ? 1000 : code, reason);
		} catch {
			// already closed
		}
	}

	webSocketError(ws: WebSocket): void {
		this.onGone(ws);
	}

	private onGone(ws: WebSocket): void {
		if (this.ctx.getTags(ws).includes("host")) {
			const att = ws.deserializeAttachment() as HostAttachment;
			if (att.authed) this.hostGone(ws);
			return;
		}
		const att = ws.deserializeAttachment() as ViewerAttachment | null;
		if (att?.closedByServer) return;
		const host = this.authedHost();
		host?.send(JSON.stringify({ t: "viewer_left" }));
	}

	/** Marks the host as no longer waiting and ends the viewer session. */
	private hostGone(host: WebSocket): void {
		const att = host.deserializeAttachment() as HostAttachment;
		host.serializeAttachment({ ...att, authed: false } satisfies HostAttachment);
		const viewer = this.openViewer();
		if (viewer === undefined) return;
		viewer.send(JSON.stringify({ t: "host_left" }));
		this.closeFromServer(viewer, CLOSE_HOST_LEFT, "host left");
	}

	private closeFromServer(ws: WebSocket, code: number, reason: string): void {
		if (this.ctx.getTags(ws).includes("viewer")) {
			ws.serializeAttachment({ closedByServer: true } satisfies ViewerAttachment);
		}
		ws.close(code, reason);
	}

	private authedHost(): WebSocket | undefined {
		return this.ctx
			.getWebSockets("host")
			.find((ws) => ws.readyState === WebSocket.OPEN && (ws.deserializeAttachment() as HostAttachment).authed);
	}

	private openViewer(): WebSocket | undefined {
		return this.ctx.getWebSockets("viewer").find((ws) => ws.readyState === WebSocket.OPEN);
	}
}

function parse(message: string | ArrayBuffer): { t: unknown; [k: string]: unknown } | null {
	if (typeof message !== "string") return null;
	let value: unknown;
	try {
		value = JSON.parse(message);
	} catch {
		return null;
	}
	if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
	return value as { t: unknown; [k: string]: unknown };
}
