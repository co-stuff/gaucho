<!-- ••••••••••••••••••••••
 · with love from bolzov (人◕‿◕)
 · https://github.com/ibolzov
 · gaucho - conquer mobile private server
 •••••••••••••••••••••• -->

# apk

## change the ip

drag the apk onto `scripts\set_ip.bat` (or `python tools\build_apk.py <ip> --base <apk>`), out comes `<apk>_<ip>.apk`.
needs python `cryptography`, java and android build-tools (`ANDROID_BUILD_TOOLS`).

- the server list is `assets/ConquerRes/server_local.dat` + `server_world.dat`: gzipped xml, rsa signed with
  `tools/keys/server_list.pem`. the lib carries that public key instead of the official one.
- the resource cdn puts the official `server_local.dat` back after first launch, so the lib is patched to read
  `server_world.dat`.
- the ip also goes to `assets/server_host.txt`, where the login dialog hits port 9528.
- always signed with `tools/keys/release.keystore`, so `adb install -r` keeps app data.
- phones need 8020, 5816 and 9528 open. the apk is just the apk: the client pulls ~2 gb from the official cdn on
  first launch.

## build from a new official client

```
python tools/make_mod_base.py <client folder> <work>      base.apk + split_config.*.apk + split_install_time*.apk
python tools/build_apk.py <ip> --base <work>/base.apk --out Gaucho.apk
```

needs apktool (`APKTOOL`, default `tools/apktool.jar`), llvm-objdump (`OBJDUMP`) and android.jar (`ANDROID_JAR`).

what `make_mod_base.py` does:

- merges the splits into one apk, renames it to `com.gaucho.game` / gaucho, drops the facebook provider (its
  authority clashes with an installed official client).
- swaps `ChannelAdapter`'s sdk init/login for `tools/mod/channel_adapter.smali`. login and switch open
  `tools/mod/AccountLogin.java`, built with javac + d8 into one more `classesN.dex`.
- lib: our rsa key, every `*.99.com` url to 127.0.0.1:9528 (`ipadco.99.com/nexus` hands out the official server
  list), `server_world.dat`, and the "failed to connect to the update server" prompt removed.
- `res.dat` = 1123: the client sends it on connect, the server uses it to know this build (account login, cutscenes
  answered with the next plot because 1.1.2.3 hangs on them).

`AccountLogin.java` uses flat anonymous classes on purpose: android-34's `android.jar` has no `LambdaMetafactory`
and d8 8.2 crashes on nested anonymous classes compiled by javac 21.
