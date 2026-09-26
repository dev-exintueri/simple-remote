import { describe, expect, it } from "vitest";
import { connect, freshIp, newKey, registerHost, request, setMode, sign } from "./helpers";

describe("host registration", () => {
	it("assigns_nine_digit_id_and_binds_key", async () => {
		const key = await newKey();
		const { id } = await registerHost(key, freshIp());
		expect(id).toMatch(/^[1-9]\d{8}$/);
	});

	it("same_key_keeps_id", async () => {
		const key = await newKey();
		const first = await registerHost(key, freshIp());
		first.peer.ws.close(1000, "bye");
		const again = await registerHost(key, freshIp(), first.id);
		expect(again.id).toBe(first.id);
	});

	it("accepts uppercase hex key for the same id", async () => {
		const key = await newKey();
		const first = await registerHost(key, freshIp());
		const again = await registerHost({ pub: key.pub.toUpperCase(), priv: key.priv }, freshIp(), first.id);
		expect(again.id).toBe(first.id);
	});

	it("other_key_is_forbidden", async () => {
		const owner = await newKey();
		const other = await newKey();
		const { id } = await registerHost(owner, freshIp());
		const res = await request(`/v1/host/${id}?key=${other.pub}`, freshIp());
		expect(res.status).toBe(403);
	});

	it("unknown_id_is_not_found_for_host", async () => {
		const key = await newKey();
		const res = await request(`/v1/host/123456789?key=${key.pub}`, freshIp());
		expect(res.status).toBe(404);
	});

	it("bad_signature_closes_4003", async () => {
		const key = await newKey();
		const other = await newKey();
		const peer = await connect(`/v1/host?key=${key.pub}`, freshIp());
		const challenge = await peer.next();
		peer.send({ t: "auth", sig: await sign(other.priv, challenge.nonce, challenge.id) });
		expect(await peer.closeCode()).toBe(4003);
	});

	it("rejects_short_signature", async () => {
		const key = await newKey();
		const peer = await connect(`/v1/host?key=${key.pub}`, freshIp());
		const challenge = await peer.next();
		const sig = await sign(key.priv, challenge.nonce, challenge.id);
		peer.send({ t: "auth", sig: sig.slice(0, 126) });
		expect(await peer.closeCode()).toBe(4003);
	});

	it("rejects non-hex signature", async () => {
		const key = await newKey();
		const peer = await connect(`/v1/host?key=${key.pub}`, freshIp());
		await peer.next();
		peer.send({ t: "auth", sig: "zz".repeat(64) });
		expect(await peer.closeCode()).toBe(4003);
	});

	it("message before auth closes 4003", async () => {
		const key = await newKey();
		const peer = await connect(`/v1/host?key=${key.pub}`, freshIp());
		await peer.next();
		peer.send({ t: "relay", data: "00" });
		expect(await peer.closeCode()).toBe(4003);
	});

	it("rejects_missing_client_ip", async () => {
		const key = await newKey();
		const res = await request(`/v1/host?key=${key.pub}`, null);
		expect(res.status).toBe(400);
	});

	it("host_rate_limited_429", async () => {
		const ip = freshIp();
		for (let i = 0; i < 20; i++) {
			expect((await request(`/v1/host?key=bad`, ip)).status).toBe(400);
		}
		expect((await request(`/v1/host?key=bad`, ip)).status).toBe(429);
	});

	it("rejects malformed key", async () => {
		const res = await request(`/v1/host?key=abcd`, freshIp());
		expect(res.status).toBe(400);
	});

	it("unknown path is 404 and missing upgrade is 426", async () => {
		expect((await request("/v1/nothing", freshIp())).status).toBe(404);
		expect((await request("/v1/viewer/12345", freshIp())).status).toBe(404);
		const key = await newKey();
		expect((await request(`/v1/host?key=${key.pub}`, freshIp(), false)).status).toBe(426);
	});

	it("new_host_connection_replaces_old", async () => {
		const key = await newKey();
		const first = await registerHost(key, freshIp());
		const second = await registerHost(key, freshIp(), first.id);
		expect(await first.peer.closeCode()).toBe(4010);

		setMode(second.peer, "new");
		const viewer = await connect(`/v1/viewer/${first.id}?kind=new`, freshIp());
		expect(await viewer.next()).toEqual({ t: "joined" });
		expect(await second.peer.next()).toEqual({ t: "viewer_joined", n: 1, kind: "new" });
	});

	it("unauthenticated_host_is_not_waiting", async () => {
		const key = await newKey();
		const first = await registerHost(key, freshIp());
		first.peer.ws.close(1000, "bye");
		await first.peer.closeCode();
		const pending = await connect(`/v1/host/${first.id}?key=${key.pub}`, freshIp());
		expect((await pending.next()).t).toBe("challenge");
		const res = await request(`/v1/viewer/${first.id}?kind=new`, freshIp());
		expect(res.status).toBe(404);
	});
});
