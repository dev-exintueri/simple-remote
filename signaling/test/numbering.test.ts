import { describe, expect, it } from "vitest";
import { connect, freshIp, newKey, registerHost, request, setMode } from "./helpers";

describe("viewer numbering, join kind, host mode", () => {
	it("viewer_numbers_increase", async () => {
		const key = await newKey();
		const host = await registerHost(key, freshIp());
		setMode(host.peer, "new");

		const v1 = await connect(`/v1/viewer/${host.id}?kind=new`, freshIp());
		expect(await v1.next()).toEqual({ t: "joined" });
		expect(await host.peer.next()).toEqual({ t: "viewer_joined", n: 1, kind: "new" });
		v1.ws.close(1000, "bye");
		expect(await host.peer.next()).toEqual({ t: "viewer_left", n: 1 });

		const v2 = await connect(`/v1/viewer/${host.id}?kind=new`, freshIp());
		expect(await v2.next()).toEqual({ t: "joined" });
		expect(await host.peer.next()).toEqual({ t: "viewer_joined", n: 2, kind: "new" });
		v2.ws.close(1000, "bye");
		expect(await host.peer.next()).toEqual({ t: "viewer_left", n: 2 });
	});

	it("stale_kick_and_relay_are_ignored", async () => {
		const key = await newKey();
		const host = await registerHost(key, freshIp());
		setMode(host.peer, "new");

		const v1 = await connect(`/v1/viewer/${host.id}?kind=new`, freshIp());
		expect(await v1.next()).toEqual({ t: "joined" });
		expect(await host.peer.next()).toEqual({ t: "viewer_joined", n: 1, kind: "new" });
		v1.ws.close(1000, "bye");
		expect(await host.peer.next()).toEqual({ t: "viewer_left", n: 1 });

		const v2 = await connect(`/v1/viewer/${host.id}?kind=new`, freshIp());
		expect(await v2.next()).toEqual({ t: "joined" });
		expect(await host.peer.next()).toEqual({ t: "viewer_joined", n: 2, kind: "new" });

		host.peer.send({ t: "kick", n: 1 });
		host.peer.send({ t: "relay", n: 1, data: "aa" });
		expect(await v2.silentFor(200)).toBe(true);

		host.peer.send({ t: "relay", n: 2, data: "bb" });
		expect(await v2.next()).toEqual({ t: "relay", data: "bb" });

		host.peer.send({ t: "kick", n: 2 });
		expect(await v2.closeCode()).toBe(4001);
	});

	it("mode_gates_join_kind", async () => {
		const key = await newKey();
		const host = await registerHost(key, freshIp());

		expect((await request(`/v1/viewer/${host.id}?kind=new`, freshIp())).status).toBe(404);

		const reconnectPeer = await connect(`/v1/viewer/${host.id}?kind=reconnect`, freshIp());
		expect(await reconnectPeer.next()).toEqual({ t: "joined" });
		expect(await host.peer.next()).toEqual({ t: "viewer_joined", n: 1, kind: "reconnect" });
		reconnectPeer.ws.close(1000, "bye");
		expect(await host.peer.next()).toEqual({ t: "viewer_left", n: 1 });

		setMode(host.peer, "new");
		const newPeer = await connect(`/v1/viewer/${host.id}?kind=new`, freshIp());
		expect(await newPeer.next()).toEqual({ t: "joined" });
		expect(await host.peer.next()).toEqual({ t: "viewer_joined", n: 2, kind: "new" });
	});

	it("missing_or_bad_kind_400", async () => {
		const key = await newKey();
		const host = await registerHost(key, freshIp());
		expect((await request(`/v1/viewer/${host.id}`, freshIp())).status).toBe(400);
		expect((await request(`/v1/viewer/${host.id}?kind=bogus`, freshIp())).status).toBe(400);
	});

	it("kick_without_n_closes_host_4000", async () => {
		const key = await newKey();
		const host = await registerHost(key, freshIp());
		host.peer.send({ t: "kick" });
		expect(await host.peer.closeCode()).toBe(4000);
	});

	it("bad_mode_closes_4000", async () => {
		const key = await newKey();
		const host = await registerHost(key, freshIp());
		host.peer.send({ t: "mode", mode: "bogus" });
		expect(await host.peer.closeCode()).toBe(4000);
	});
});
