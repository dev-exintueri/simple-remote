import { DurableObject } from "cloudflare:workers";

export class Room extends DurableObject<Env> {
	fetch(request: Request): Response {
		if (request.headers.get("Upgrade") !== "websocket") {
			return new Response("expected websocket", { status: 426 });
		}
		const { 0: client, 1: server } = new WebSocketPair();
		// Hibernation API: the runtime owns the socket, DO may be evicted between messages.
		this.ctx.acceptWebSocket(server);
		return new Response(null, { status: 101, webSocket: client });
	}

	webSocketMessage(ws: WebSocket, message: string | ArrayBuffer): void {
		for (const other of this.ctx.getWebSockets()) {
			if (other !== ws) other.send(message);
		}
	}

	webSocketClose(ws: WebSocket, code: number, reason: string): void {
		ws.close(code, reason);
	}
}

export default {
	fetch(request, env) {
		const match = new URL(request.url).pathname.match(/^\/room\/([^/]+)$/);
		if (request.method !== "GET" || match === null) {
			return new Response("not found", { status: 404 });
		}
		const stub = env.ROOM.get(env.ROOM.idFromName(match[1]));
		return stub.fetch(request);
	},
} satisfies ExportedHandler<Env>;
