import { describe, expect, it } from "vitest";
import { connect, freshIp, newKey, registerHost, request } from "./helpers";

async function pair() {
	const key = await newKey();
	const host = await registerHost(key, freshIp());
	const viewer = await connect(`/v1/viewer/${host.id}`, freshIp());
	expect(await viewer.next()).toEqual({ t: "joined" });
	expect(await host.peer.next()).toEqual({ t: "viewer_joined" });
	return { host: host.peer, id: host.id, viewer };
}

describe("viewer and relay", () => {
	it("viewer_to_offline_host_404", async () => {
		const key = await newKey();
		const host = await registerHost(key, freshIp());
		host.peer.ws.close(1000, "bye");
		await host.peer.closeCode();
		expect((await request(`/v1/viewer/${host.id}`, freshIp())).status).toBe(404);
		expect((await request(`/v1/viewer/987654321`, freshIp())).status).toBe(404);
	});

	it("relays_both_ways", async () => {
		const { host, viewer } = await pair();
		host.send({ t: "relay", data: "00ff10" });
		expect(await viewer.next()).toEqual({ t: "relay", data: "00ff10" });
		viewer.send({ t: "relay", data: "abcdef" });
		expect(await host.next()).toEqual({ t: "relay", data: "abcdef" });
	});

	it("second_viewer_409", async () => {
		const { id } = await pair();
		expect((await request(`/v1/viewer/${id}`, freshIp())).status).toBe(409);
	});

	it("kick_closes_viewer_4001", async () => {
		const { host, viewer, id } = await pair();
		host.send({ t: "kick" });
		expect(await viewer.closeCode()).toBe(4001);
		expect(await host.silentFor(200)).toBe(true);
		// the slot is free again
		const next = await connect(`/v1/viewer/${id}`, freshIp());
		expect(await next.next()).toEqual({ t: "joined" });
	});

	it("relay_from_kicked_viewer_never_reaches_host", async () => {
		const { host, viewer, id } = await pair();
		host.send({ t: "kick" });
		// Sent before the client has seen the server's close frame: the server has
		// already closed this socket when the message is handled.
		viewer.send({ t: "relay", data: "a1" });
		expect(await viewer.closeCode()).toBe(4001);
		try {
			viewer.send({ t: "relay", data: "a2" });
		} catch {
			// the runtime may refuse sends on a closed socket
		}
		const next = await connect(`/v1/viewer/${id}`, freshIp());
		expect(await next.next()).toEqual({ t: "joined" });
		expect(await host.next()).toEqual({ t: "viewer_joined" });
		next.send({ t: "relay", data: "b1" });
		expect(await host.next()).toEqual({ t: "relay", data: "b1" });
		expect(await host.silentFor(200)).toBe(true);
	});

	it("host_close_sends_host_left", async () => {
		const { host, viewer } = await pair();
		host.ws.close(1000, "bye");
		expect(await viewer.next()).toEqual({ t: "host_left" });
		expect(await viewer.closeCode()).toBe(4002);
	});

	it("viewer_close_sends_viewer_left", async () => {
		const { host, viewer } = await pair();
		viewer.ws.close(1000, "bye");
		expect(await host.next()).toEqual({ t: "viewer_left" });
	});

	it("oversize_message_closes_4013", async () => {
		const { host } = await pair();
		host.send({ t: "relay", data: "a".repeat(65_536) });
		expect(await host.closeCode()).toBe(4013);
	});

	it("viewer non-relay message closes 4000", async () => {
		const { viewer } = await pair();
		viewer.send({ t: "kick" });
		expect(await viewer.closeCode()).toBe(4000);
	});

	it("invalid json closes 4000", async () => {
		const { host } = await pair();
		host.send("not json");
		expect(await host.closeCode()).toBe(4000);
	});

	it("host relay without viewer is ignored", async () => {
		const key = await newKey();
		const { peer } = await registerHost(key, freshIp());
		peer.send({ t: "relay", data: "00" });
		peer.send({ t: "kick" });
		expect(await peer.silentFor(200)).toBe(true);
	});

	it("viewer_rate_limited_429", async () => {
		const ip = freshIp();
		for (let i = 0; i < 30; i++) {
			expect((await request(`/v1/viewer/111111111`, ip)).status).toBe(404);
		}
		expect((await request(`/v1/viewer/111111111`, ip)).status).toBe(429);
	});
});
