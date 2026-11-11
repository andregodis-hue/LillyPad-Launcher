# Builds and signs release.json for the launcher. Usage: python3 release.py <mrpack file> <web dir>
# The mrpack must already be in <resource dir>/pack/releases/. The private key stays in launcher/keys, never on the web server.
import hashlib, json, os, subprocess, sys, tempfile, urllib.request, zipfile
mrpack, webdir = sys.argv[1], sys.argv[2]
here = os.path.dirname(os.path.abspath(__file__))
z = zipfile.ZipFile(mrpack)
idx = json.loads(z.read("modrinth.index.json"))
mc, nf, version = idx["dependencies"]["minecraft"], idx["dependencies"]["neoforge"], idx["versionId"]
data = open(mrpack, "rb").read()
inst_url = f"https://maven.neoforged.net/releases/net/neoforged/neoforge/{nf}/neoforge-{nf}-installer.jar"
inst = urllib.request.urlopen(urllib.request.Request(inst_url, headers={"User-Agent": "lillypad-release/1"})).read()
rel = {
    "version": version, "minecraft": mc, "neoforge": nf,
    "neoforge_installer_sha256": hashlib.sha256(inst).hexdigest(),
    "mrpack": {"url": f"https://lillypadnetwork.se/pack/releases/{os.path.basename(mrpack)}", "sha256": hashlib.sha256(data).hexdigest(), "size": len(data)},
    "news": json.load(open(os.path.join(here, "news.json"))),
}
out = os.path.join(webdir, "release.json")
open(out, "w").write(json.dumps(rel, indent=1, ensure_ascii=False))
subprocess.check_call(["minisign", "-S", "-s", os.environ.get("LILLYPAD_PACK_KEY", os.path.join(here, "..", "keys", "pack.key")), "-m", out, "-x", out + ".minisig"])
print(version, mc, nf, rel["mrpack"]["sha256"][:12])
