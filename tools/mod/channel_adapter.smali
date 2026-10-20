#+++++++++++++++
# * gaucho - conquer mobile private server
# * with love from bolzov ʕ•ᴥ•ʔ
# * https://github.com/ibolzov
#+++++++++++++++

.method public static ChannelInit()Z
    .locals 1

    const/4 v0, 0x1
    sput-boolean v0, Lcom/Tq/CQ2ClientAndroid/ChannelAdapter;->bInitSDK:Z

    invoke-static {v0}, Lcom/Tq/C3Engine/RelayNative;->nativeChannelCommonSdkOnInit(I)V

    sget-boolean v0, Lcom/Tq/CQ2ClientAndroid/ChannelAdapter;->bInitToLogin:Z
    if-eqz v0, :skip_login
    invoke-static {}, Lcom/Tq/CQ2ClientAndroid/ChannelAdapter;->ChannelLogin()V

    :skip_login
    const/4 v0, 0x1
    return v0
.end method

.method public static ChannelLogin()V
    .locals 1

    sget-object v0, Lcom/Tq/CQ2ClientAndroid/ChannelAdapter;->sContext:Landroid/app/Activity;
    invoke-static {v0}, Lcom/Tq/CQ2ClientAndroid/AccountLogin;->login(Landroid/app/Activity;)V

    return-void
.end method

.method public static ChannelSwitchAccount()V
    .locals 1

    sget-object v0, Lcom/Tq/CQ2ClientAndroid/ChannelAdapter;->sContext:Landroid/app/Activity;
    invoke-static {v0}, Lcom/Tq/CQ2ClientAndroid/AccountLogin;->switchAccount(Landroid/app/Activity;)V

    return-void
.end method

.method public static GetUnionToken()V
    .locals 1

    const-string v0, "private_token"
    invoke-static {v0}, Lcom/Tq/C3Engine/RelayNative;->nativeSetUnionToken(Ljava/lang/String;)V

    return-void
.end method

.method public static getCustomerUrl(Z)Ljava/lang/String;
    .locals 0

    const-string p0, "http://127.0.0.1:9528/v3/handlerclient/"

    return-object p0
.end method
