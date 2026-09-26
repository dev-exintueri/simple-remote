import { fromHex } from "./hex";

/** Verifies an Ed25519 signature. Any malformed input or runtime error yields false (fail closed). */
export async function verifyEd25519(pubHex: string, sigHex: string, msg: Uint8Array): Promise<boolean> {
	const pub = fromHex(pubHex);
	const sig = fromHex(sigHex);
	if (pub === null || sig === null || pub.length !== 32 || sig.length !== 64) return false;
	try {
		const key = await crypto.subtle.importKey("raw", pub, { name: "Ed25519" }, false, ["verify"]);
		return await crypto.subtle.verify({ name: "Ed25519" }, key, sig, msg);
	} catch {
		return false;
	}
}
