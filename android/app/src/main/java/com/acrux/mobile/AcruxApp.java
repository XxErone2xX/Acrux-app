package com.acrux.mobile;

import android.app.Application;

/** 앱이 켜질 때 가장 먼저: 크래시 기록 준비 (화면 · 서비스보다 먼저) */
public class AcruxApp extends Application {
    @Override
    public void onCreate() {
        super.onCreate();
        Crash.install(this);
        Quiet.off(this);                                          // 매크로 중에 앱이 강제로 꺼졌으면 켜 둔 방해 금지를 되돌림
    }
}
