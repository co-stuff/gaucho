/*············
 ~ gaucho - conquer mobile private server
 ~ with love from bolzov (●'◡'●)
 ~ https://github.com/ibolzov
 ············*/

package com.Tq.CQ2ClientAndroid;

import android.app.Activity;
import android.app.Dialog;
import android.content.Context;
import android.content.SharedPreferences;
import android.util.Log;
import android.view.View;
import android.widget.EditText;
import android.widget.Toast;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.security.MessageDigest;

public final class AccountLogin {
    private static final String APP_ID = "200016";
    private static final int PORT = 9528;
    private static boolean busy;

    public static void login(Activity a) {
        start(a, false);
    }

    public static void switchAccount(Activity a) {
        start(a, true);
    }

    private static void start(final Activity a, final boolean switching) {
        a.runOnUiThread(new Runnable() {
            public void run() {
                SharedPreferences p = prefs(a);
                String acc = p.getString("account", ""), pass = p.getString("pass", "");
                if (!switching && !acc.isEmpty() && !pass.isEmpty()) {
                    send(a, null, acc, pass, false, false);
                } else {
                    splash(a, switching);
                }
            }
        });
    }

    private static void splash(final Activity a, final boolean switching) {
        final Dialog d = dialog(a, "ydzf_nd_dialog_account_third_login_lite", switching);
        hide(d, "nd_customer_center", "nd_ll_google", "nd_ll_fb", "nd_ll_guest_parent");
        view(d, "nd_other_account_login").setOnClickListener(new View.OnClickListener() {
            public void onClick(View v) {
                d.dismiss();
                form(a, switching);
            }
        });
        d.show();
    }

    private static void form(final Activity a, final boolean switching) {
        final Dialog d = dialog(a, "ydzf_nd_dialog_account_login", switching);
        hide(d, "nd_iv_logo", "nd_iv_pull_down", "nd_forget_pwd", "nd_login_tv_version");
        final EditText acc = (EditText) view(d, "nd_et_login_account"), pass = (EditText) view(d, "nd_et_login_password");
        acc.setText(prefs(a).getString("account", ""));
        view(d, "nd_iv_dialog_close").setOnClickListener(new View.OnClickListener() {
            public void onClick(View v) {
                d.dismiss();
                if (!switching) {
                    splash(a, false);
                }
            }
        });
        view(d, "nd_tv_confirm_btn_confirm").setOnClickListener(new View.OnClickListener() {
            public void onClick(View v) {
                submit(a, d, acc, pass, false, switching);
            }
        });
        view(d, "nd_tv_fast_register_link").setOnClickListener(new View.OnClickListener() {
            public void onClick(View v) {
                submit(a, d, acc, pass, true, switching);
            }
        });
        d.show();
    }

    private static void submit(Activity a, Dialog d, EditText acc, EditText pass, boolean register, boolean switching) {
        String name = acc.getText().toString().trim(), pw = pass.getText().toString();
        if (name.isEmpty() || pw.isEmpty()) {
            Toast.makeText(a, "Enter your account and password", Toast.LENGTH_SHORT).show();
            return;
        }
        send(a, d, name, md5(pw), register, switching);
    }

    private static void send(final Activity a, final Dialog d, final String acc, final String pass,
                             final boolean register, final boolean switching) {
        if (busy) {
            return;
        }
        busy = true;
        new Thread(new Runnable() {
            public void run() {
                String r;
                try {
                    r = get(a, acc, pass, register);
                } catch (Exception e) {
                    r = "err Cannot reach the server";
                }
                post(a, d, pass, r, switching);
            }
        }).start();
    }

    private static void post(final Activity a, final Dialog d, final String pass, final String res, final boolean switching) {
        a.runOnUiThread(new Runnable() {
            public void run() {
                done(a, d, pass, res, switching);
            }
        });
    }

    private static void done(Activity a, Dialog d, String pass, String res, boolean switching) {
        busy = false;
        String[] ok = res.split(" ");
        if (ok.length < 3 || !ok[0].equals("ok")) {
            Toast.makeText(a, res.startsWith("err ") ? res.substring(4) : res, Toast.LENGTH_LONG).show();
            if (d == null) {
                splash(a, switching);
            }
            return;
        }
        prefs(a).edit().putString("account", ok[1]).putString("pass", pass).apply();
        if (d != null) {
            d.dismiss();
        }
        String method = switching ? "nativeChannelCommonSdkOnSwitchAccount" : "nativeChannelCommonSdkOnLogin";
        try {
            Class.forName("com.Tq.C3Engine.RelayNative")
                    .getMethod(method, int.class, String.class, String.class, String.class, String.class)
                    .invoke(null, 1, ok[1], APP_ID, ok[2], "");
        } catch (Exception e) {
            Log.e("AccountLogin", method, e);
        }
    }

    private static String get(Context c, String acc, String pass, boolean register) throws Exception {
        URL u = new URL("http://" + host(c) + ":" + PORT + "/" + (register ? "register" : "login")
                + "?a=" + URLEncoder.encode(acc, "UTF-8") + "&p=" + pass);
        HttpURLConnection h = (HttpURLConnection) u.openConnection();
        h.setConnectTimeout(5000);
        h.setReadTimeout(5000);
        try (BufferedReader r = new BufferedReader(new InputStreamReader(h.getInputStream(), "UTF-8"))) {
            String line = r.readLine();
            return line == null ? "" : line.trim();
        } finally {
            h.disconnect();
        }
    }

    private static String host(Context c) {
        try (BufferedReader r = new BufferedReader(new InputStreamReader(c.getAssets().open("server_host.txt")))) {
            String h = r.readLine();
            if (h != null && !h.trim().isEmpty()) {
                return h.trim();
            }
        } catch (Exception ignored) {
        }
        return "127.0.0.1";
    }

    private static String md5(String s) {
        try {
            StringBuilder b = new StringBuilder();
            for (byte x : MessageDigest.getInstance("MD5").digest(s.getBytes("UTF-8"))) {
                b.append(String.format("%02x", x));
            }
            return b.toString();
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }

    private static SharedPreferences prefs(Context c) {
        return c.getSharedPreferences("account_login", Context.MODE_PRIVATE);
    }

    private static Dialog dialog(Activity a, String layout, boolean cancelable) {
        Dialog d = new Dialog(a, android.R.style.Theme_Translucent_NoTitleBar_Fullscreen);
        d.setContentView(res(a, layout, "layout"));
        d.setCancelable(cancelable);
        d.getWindow().getDecorView().setSystemUiVisibility(a.getWindow().getDecorView().getSystemUiVisibility());
        return d;
    }

    private static int res(Context c, String name, String type) {
        int id = c.getResources().getIdentifier(name, type, c.getPackageName());
        return id != 0 ? id : c.getResources().getIdentifier(name, type, "com.Tq.CQ2ClientAndroid");
    }

    private static View view(Dialog d, String id) {
        return d.findViewById(res(d.getContext(), id, "id"));
    }

    private static void hide(Dialog d, String... ids) {
        for (String id : ids) {
            View v = view(d, id);
            if (v != null) {
                v.setVisibility(View.GONE);
            }
        }
    }
}
