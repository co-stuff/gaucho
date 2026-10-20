<!-- ***********************
 > with love from bolzov (●'◡'●)
 > https://github.com/ibolzov
 > gaucho - conquer mobile private server
 *********************** -->

# accounts

- login with an unknown account creates it. register does the same but fails if the name is taken with another
  password.
- no characters: "start game" opens character creation. otherwise you get the last one played.
- the client remembers the last account and logs in on its own. "switch" on the login screen brings the splash back.
- in game: `/characters`, `/switch [name]`, `/new`.

## how

- `tools/mod/AccountLogin.java` inflates the sdk's own layouts (`ydzf_nd_dialog_account_third_login_lite`,
  `ydzf_nd_dialog_account_login`). they only use stock widgets, so they work with the sdk switched off.
- `GET http://<ip>:9528/login` (or `/register`) `?a=<account>&p=<md5 of password>` -> `ok <account> <token>` or
  `err <message>`. on ok it calls `nativeChannelCommonSdkOnLogin` (`...OnSwitchAccount` from switch), same as the sdk
  callback.
- `data/accounts.json` stores the md5 of what the client sends.
- the account reaches the game by ip: the login binds it to the caller's ip, and the next game connection from that
  ip with `res.dat` 1123 uses it. two clients behind one ip logging in at the same moment can swap accounts; carrying
  the account through the auth server's srp step (0x04bd/0x04be) would fix it.
- clients without the dialog (fixed token) are identified by ip only.
