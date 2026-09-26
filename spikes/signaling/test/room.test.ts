import { runInDurableObject } from "cloudflare:test";
import { env, exports } from "cloudflare:workers";
import { expect, it } from "vitest";

async function connect(roomId: string): Promise<WebSocket> {
	const res = await exports.default.fetch(`https://example.com/room/${roomId}`, {
		headers: { Upgrade: "websocket" },
	});
	expect(res.status).toBe(101);
	const ws = res.webSocket;
	if (!ws) throw new Error("no webSocket on response");
	ws.accept();
	return ws;
}

function nextMessage(ws: WebSocket): Promise<string> {
	return new Promise((resolve, reject) => {
		const timer = setTimeout(() => reject(new Error("timeout")), 5_000);
		ws.addEventListener("message", (e) => {
			clearTimeout(timer);
			resolve(typeof e.data === "string" ? e.data : new TextDecoder().decode(e.data as ArrayBuffer));
		}, { once: true });
	});
}

it("relays a message from A to B in the same room", async () => {
	const a = await connect("r1");
	const b = await connect("r1");
	const received = nextMessage(b);
	a.send("hello");
	expect(await received).toBe("hello");
	a.close(1000, "done");
	b.close(1000, "done");
});

it("does not relay across rooms", async () => {
	const a = await connect("r2");
	const c = await connect("r3");
	let got = false;
	c.addEventListener("message", () => { got = true; });
	a.send("hello");
	await new Promise((r) => setTimeout(r, 200));
	expect(got).toBe(false);
});

it("rejects non-websocket requests", async () => {
	const res = await exports.default.fetch("https://example.com/room/r4");
	expect(res.status).toBe(426);
});

it("Room holds both accepted sockets in hibernation state", async () => {
	await connect("r5");
	await connect("r5");
	const stub = env.ROOM.get(env.ROOM.idFromName("r5"));
	const count = await runInDurableObject(stub, (_instance, state) => state.getWebSockets().length);
	expect(count).toBe(2);
});
