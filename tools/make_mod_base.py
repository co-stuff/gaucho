#===============
# + with love from bolzov (づ◕‿◕)づ
# + https://github.com/ibolzov
# + gaucho - conquer mobile private server
#===============

import argparse
import glob
import os
import re
import shutil
import struct
import subprocess
import sys
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
APKTOOL = os.environ.get("APKTOOL", os.path.join(HERE, "apktool.jar"))
BUILD_TOOLS = os.environ.get("ANDROID_BUILD_TOOLS", r"D:\AndroidSDK\build-tools\34.0.0")
OBJDUMP = os.environ.get("OBJDUMP", r"C:\Program Files\LLVM\bin\llvm-objdump.exe")
ANDROID_JAR = os.environ.get("ANDROID_JAR", r"D:\AndroidSDK\platforms\android-34\android.jar")
OG_PACKAGE, PACKAGE, LABEL = "com.Tq.CQ2ClientAndroid", "com.gaucho.game", "Gaucho"
LIB = os.path.join("lib", "arm64-v8a", "libCQ2ClientEn.so")
ADAPTER = os.path.join(HERE, "mod", "channel_adapter.smali")
LOGIN_JAVA = os.path.join(HERE, "mod", "AccountLogin.java")
SERVER_LIST = "server_world"
RES_MARK = "1123"
LOGIN_LABEL = "Login"
OG_MODULUS = open(os.path.join(HERE, "mod", "og_rsa_modulus.txt"), "rb").read().strip()
METHOD = re.compile(r"^\.method [^\n]*?(\S+\(.*?\)\S*)\n.*?^\.end method", re.M | re.S)


def run(*cmd):
    print(">", " ".join(str(c) for c in cmd[:4]), "...")
    subprocess.run(cmd, check=True)


def unzip(apk, prefix, dest):
    with zipfile.ZipFile(apk) as z:
        for n in z.namelist():
            if n.startswith(prefix) and not n.endswith("/"):
                out = os.path.join(dest, n)
                os.makedirs(os.path.dirname(out), exist_ok=True)
                with z.open(n) as src, open(out, "wb") as dst:
                    shutil.copyfileobj(src, dst)


def patch_smali(dec):
    path = glob.glob(os.path.join(dec, "smali*", "com", "Tq", "CQ2ClientAndroid", "ChannelAdapter.smali"))[0]
    text = open(path, encoding="utf-8").read().replace("\r\n", "\n")
    ours = {m.group(1): m.group(0) for m in METHOD.finditer(open(ADAPTER, encoding="utf-8").read())}
    text = METHOD.sub(lambda m: ours.get(m.group(1), m.group(0)), text)
    missing = set(ours) - {m.group(1) for m in METHOD.finditer(text)}
    if missing:
        sys.exit(f"ChannelAdapter lacks {missing}")
    open(path, "w", encoding="utf-8", newline="\n").write(text)
    url = re.compile(r"https?://[A-Za-z0-9.-]*\b99\.com")
    count = 0
    for f in glob.glob(os.path.join(dec, "smali*", "com", "**", "*.smali"), recursive=True):
        t = open(f, encoding="utf-8").read()
        new, k = url.subn("http://127.0.0.1:9528", t)
        if k:
            open(f, "w", encoding="utf-8", newline="\n").write(new)
            count += k
    print(f"ChannelAdapter replaced, {count} SDK urls redirected")


def patch_manifest(dec):
    path = os.path.join(dec, "AndroidManifest.xml")
    t = open(path, encoding="utf-8").read()
    t = re.sub(r' android:(requiredSplitTypes|splitTypes)="[^"]*"', "", t)
    t = t.replace('android:extractNativeLibs="false"', 'android:extractNativeLibs="true"')
    t = t.replace(f'package="{OG_PACKAGE}"', f'package="{PACKAGE}"')
    og = re.escape(OG_PACKAGE)
    t = re.sub(rf'android:authorities="{og}\.', f'android:authorities="{PACKAGE}.', t)
    t = re.sub(rf'"{og}\.(?=[A-Z_]+_PERMISSION"|action\.)', f'"{PACKAGE}.', t)
    t = t.replace(f'android:host="cct.{OG_PACKAGE}"', f'android:host="cct.{PACKAGE}"')
    t = re.sub(r'<provider android:authorities="com\.facebook\.app\.FacebookContentProvider\d+"[^>]*/>', "", t)
    open(path, "w", encoding="utf-8").write(t)
    print("manifest renamed; remaining original names (classes):",
          sorted(set(re.findall(rf'"{re.escape(OG_PACKAGE)}\.[^"]*"', t))))
    for f in glob.glob(os.path.join(dec, "res", "values*", "strings.xml")):
        s = open(f, encoding="utf-8").read()
        new = re.sub(r'(<string name="app_name_new">)[^<]*(</string>)', rf"\g<1>{LABEL}\g<2>", s)
        if new != s:
            open(f, "w", encoding="utf-8").write(new)


def shrink(xml, anchor):
    at = xml.index(anchor)
    start, end = xml.rindex("<", 0, at), xml.index(">", at)
    tag = re.sub(r' android:(layout_width|layout_height|visibility)="[^"]*"', "", xml[start:end])
    tag = tag.replace(" ", ' android:visibility="gone" android:layout_width="0.0dip" android:layout_height="0.0dip" ', 1)
    return xml[:start] + tag + xml[end:]


def patch_sdk_login(dec):
    for f in glob.glob(os.path.join(dec, "res", "values*", "strings.xml")):
        s = open(f, encoding="utf-8").read()
        new = re.sub(r'(<string name="(?:cop_nd_account_login_text|cop_nd_login_title|nd_99_account_login)">)[^<]*(</string>)',
                     rf"\g<1>{LOGIN_LABEL}\g<2>", s)
        if new != s:
            open(f, "w", encoding="utf-8").write(new)
    lay = os.path.join(dec, "res", "layout")
    splash = os.path.join(lay, "ydzf_nd_dialog_account_third_login_lite.xml")
    x = open(splash, encoding="utf-8").read()
    for anchor in ('android:id="@id/nd_ll_google"', 'android:id="@id/nd_ll_fb"', 'android:id="@id/nd_ll_guest_parent"'):
        x = shrink(x, anchor)
    open(splash, "w", encoding="utf-8").write(shrink(x, 'android:src="@null"'))
    dialog = os.path.join(lay, "ydzf_nd_dialog_account_login.xml")
    x = open(dialog, encoding="utf-8").read()
    open(dialog, "w", encoding="utf-8").write(shrink(x, 'android:id="@id/nd_iv_logo"'))
    print(f"SDK login splash: only '{LOGIN_LABEL}' (Google/Facebook/Guest and the 99 logo hidden)")


def login_dex(work):
    out = os.path.join(work, "login_dex")
    shutil.rmtree(out, ignore_errors=True)
    classes = os.path.join(out, "classes")
    run("javac", "-nowarn", "-Xlint:-options", "-source", "8", "-target", "8", "-bootclasspath", ANDROID_JAR,
        "-d", classes, LOGIN_JAVA)
    jar = os.path.join(out, "login.jar")
    with zipfile.ZipFile(jar, "w") as z:
        for f in glob.glob(os.path.join(classes, "**", "*.class"), recursive=True):
            z.write(f, os.path.relpath(f, classes))
    run(os.path.join(BUILD_TOOLS, "d8.bat"), "--release", "--min-api", "21", "--lib", ANDROID_JAR, "--output", out, jar)
    return os.path.join(out, "classes.dex")


def add_dex(apk, dex):
    with zipfile.ZipFile(apk, "a", zipfile.ZIP_DEFLATED) as z:
        name = f"classes{sum(n.startswith('classes') and n.endswith('.dex') for n in z.namelist()) + 1}.dex"
        z.write(dex, name)
    print(f"{os.path.basename(dex)} -> {name}")


def our_modulus():
    from cryptography.hazmat.primitives import serialization
    key = serialization.load_pem_private_key(open(os.path.join(HERE, "keys", "server_list.pem"), "rb").read(), None)
    return ("00" + format(key.public_key().public_numbers().n, "x").rjust(512, "0")).encode()


def no_update_patch(lib, data):
    dis = subprocess.run([OBJDUMP, "-d", "--disassemble-symbols=_ZN15CFDlgAutoUpdate10WindowProcEjjl", lib],
                         capture_output=True, text=True, check=True).stdout
    plt = subprocess.run([OBJDUMP, "-d", "-j", ".plt", lib], capture_output=True, text=True, check=True).stdout
    post = int(re.search(r"^([0-9a-f]+) <_Z7PostCmdjl@plt>:", plt, re.M).group(1), 16)
    ins = [(int(a, 16), txt) for a, txt in re.findall(r"^\s*([0-9a-f]+):\s+[0-9a-f]{8}\s+(.*)$", dis, re.M)]
    for i, (addr, txt) in enumerate(ins):
        if "mov\tw1, #0xa33" in txt and i + 10 < len(ins) and "mov\tw1, #0x2 " in ins[i + 10][1] + " ":
            start = ins[i - 6][0]
            exit_ = int(re.search(r"b\t0x([0-9a-f]+)", ins[i + 12][1]).group(1), 16)
            bl = 0x94000000 | (((post - (start + 8)) // 4) & 0x3ffffff)
            br = 0x14000000 | (((exit_ - (start + 12)) // 4) & 0x3ffffff)
            data[start:start + 16] = struct.pack("<4I", 0x5281FFE0, 0x52800041, bl, br)
            print(f"no-update patch at {start:#x} (PostCmd@plt {post:#x}, exit {exit_:#x})")
            return
    sys.exit("update hint block (type 2) not found")


def local_url(old):
    path = re.sub(rb"^https?://[^/]*", b"", old).rstrip(b"/")
    for new in (b"http://127.0.0.1:9528" + path, b"http://127.1:9528" + path, b"http://127.1:9528"):
        if len(new) <= len(old):
            return new.ljust(len(old), b"\x00")


def patch_lib(dec):
    lib = os.path.join(dec, LIB)
    data = bytearray(open(lib, "rb").read())
    at = data.find(OG_MODULUS)
    if at < 0:
        sys.exit("official RSA modulus not found in " + lib)
    data[at:at + len(OG_MODULUS)] = our_modulus()
    print(f"RSA modulus replaced at {at}")
    for m in list(re.finditer(rb"(?<=\x00)https?://[A-Za-z0-9.-]*\b99\.com[\x21-\x7e]*(?=\x00)", data)):
        old = m.group(0)
        data[m.start():m.end()] = local_url(old)
        print(f"service url {old.decode()} -> {local_url(old).rstrip(bytes(1)).decode()}")
    at = data.find(b"\x00server_local\x00")
    if at < 0:
        sys.exit("server_local name not found in " + lib)
    data[at + 1:at + 13] = SERVER_LIST.encode()
    shutil.copy(os.path.join(dec, "assets", "ConquerRes", "server_local.dat"),
                os.path.join(dec, "assets", "ConquerRes", SERVER_LIST + ".dat"))
    print(f"server list read from {SERVER_LIST}.dat (name at {at + 1:#x})")
    open(os.path.join(dec, "assets", "ConquerRes", "res.dat"), "w").write(RES_MARK)
    no_update_patch(lib, data)
    open(lib, "wb").write(data)


def main():
    ap = argparse.ArgumentParser(description="Build the unsigned mod base APK from an official client; "
                                             "build_apk.py then points it at a server and signs it.")
    ap.add_argument("og_dir", help="official client folder: base.apk + split_config.*.apk + split_install_time*.apk")
    ap.add_argument("work", help="work folder; the result is <work>/base.apk")
    ap.add_argument("--skip-decode", action="store_true", help="reuse <work>/base_decoded")
    a = ap.parse_args()
    dec = os.path.join(a.work, "base_decoded")
    os.makedirs(a.work, exist_ok=True)
    if not a.skip_decode:
        run("java", "-jar", APKTOOL, "d", "-f", os.path.join(a.og_dir, "base.apk"), "-o", dec)
    unzip(glob.glob(os.path.join(a.og_dir, "split_config.arm64_v8a.apk"))[0], "lib/", dec)
    for pack in glob.glob(os.path.join(a.og_dir, "split_install_time*.apk")):
        unzip(pack, "assets/", dec)
    hdpi = os.path.join(a.work, "hdpi_decoded")
    if not os.path.isdir(hdpi):
        subprocess.run(["java", "-jar", APKTOOL, "d", "-f", "-s", os.path.join(a.og_dir, "split_config.hdpi.apk"), "-o", hdpi])
    for d in glob.glob(os.path.join(hdpi, "res", "*")):
        if os.path.basename(d) != "values":
            shutil.copytree(d, os.path.join(dec, "res", os.path.basename(d)), dirs_exist_ok=True)
    patch_smali(dec)
    patch_sdk_login(dec)
    patch_manifest(dec)
    patch_lib(dec)
    out = os.path.join(a.work, "base.apk")
    run("java", "-jar", APKTOOL, "b", dec, "-o", out)
    add_dex(out, login_dex(a.work))
    print("base ready:", out)


if __name__ == "__main__":
    main()
