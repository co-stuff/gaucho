#=====================
# | with love from bolzov (づ◕‿◕)づ
# | https://github.com/ibolzov
# | gaucho - conquer mobile private server
#=====================

import argparse
import gzip
import os
import subprocess
import sys
import tempfile
import zipfile

from cryptography.hazmat.primitives import serialization

HERE = os.path.dirname(os.path.abspath(__file__))
KEYS = os.path.join(HERE, "keys")
BUILD_TOOLS = os.environ.get("ANDROID_BUILD_TOOLS", r"D:\AndroidSDK\build-tools\34.0.0")
DATS = ("assets/ConquerRes/server_local.dat", "assets/ConquerRes/server_world.dat")
RECOMMEND = "assets/ConquerRes/ini/recommend.ini"
RECOMMEND_OURS = b"[recommend]\r\ncount=1\r\n\r\n[server_0]\r\ngroup=0\r\nindex=0\r\nflag=0\r\n"
HOST = "assets/server_host.txt"
KEY_ALIAS, KEY_PASS = "gaucho", "gaucho123"


def server_dat(ip):
    xml = open(os.path.join(HERE, "mod", "server_list.xml"), "rb").read().replace(b"{ip}", ip.encode())
    key = serialization.load_pem_private_key(open(os.path.join(KEYS, "server_list.pem"), "rb").read(), None)
    k, data = key.private_numbers(), gzip.compress(xml)
    n, d, e = k.public_numbers.n, k.d, k.public_numbers.e
    out = b""
    for i in range(0, len(data), 244):
        block = data[i:i + 244]
        out += pow(int.from_bytes(b"\x00\x01" + b"\xff" * (253 - len(block)) + b"\x00" + block, "big"), d, n).to_bytes(256, "big")
    back = b"".join(p[p.index(b"\x00", 2) + 1:] for p in
                    (pow(int.from_bytes(out[i:i + 256], "big"), e, n).to_bytes(256, "big") for i in range(0, len(out), 256)))
    if gzip.decompress(back) != xml:
        sys.exit("server list does not verify")
    return out


def main():
    ap = argparse.ArgumentParser(description="Point the mod APK at a server IP and sign it.")
    ap.add_argument("ip")
    ap.add_argument("--base", required=True, help="APK to start from: <work>/base.apk or an already built one")
    ap.add_argument("--out", help="default: <base>_<ip>.apk next to the base")
    a = ap.parse_args()
    out = a.out or f"{os.path.splitext(a.base)[0]}_{a.ip}.apk"
    dat = server_dat(a.ip)
    tmp = tempfile.mkdtemp()
    unsigned, aligned = os.path.join(tmp, "unsigned.apk"), os.path.join(tmp, "aligned.apk")
    with zipfile.ZipFile(a.base) as src, zipfile.ZipFile(unsigned, "w") as dst:
        for info in src.infolist():
            name = info.filename
            if name == HOST or name.startswith("META-INF/") and name.endswith((".SF", ".RSA", ".EC", ".MF")):
                continue
            data = src.read(info)
            dst.writestr(info, dat if name in DATS else RECOMMEND_OURS if name == RECOMMEND else data)
        dst.writestr(HOST, a.ip)
    subprocess.run([os.path.join(BUILD_TOOLS, "zipalign.exe"), "-f", "-p", "4", unsigned, aligned], check=True)
    subprocess.run([os.path.join(BUILD_TOOLS, "apksigner.bat"), "sign", "--ks", os.path.join(KEYS, "release.keystore"),
                    "--ks-key-alias", KEY_ALIAS, "--ks-pass", f"pass:{KEY_PASS}", "--key-pass", f"pass:{KEY_PASS}",
                    "--v4-signing-enabled", "false", "--out", out, aligned], check=True)
    for f in (unsigned, aligned):
        os.remove(f)
    print(f"[OK] {out} -> {a.ip}")


if __name__ == "__main__":
    main()
