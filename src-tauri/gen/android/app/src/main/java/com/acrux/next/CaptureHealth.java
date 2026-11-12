package com.acrux.next;

import java.nio.ByteBuffer;

/** Capture delivery health, driven by monotonic milliseconds, without Android dependencies. */
final class CaptureHealth {
    static final long STALE_MS = 8000, BLACK_MS = 6000, WARN_GAP_MS = 30000;
    private long frameAt;
    private long blackSince = -1;
    private long warnedAt = -1;

    void reset(long now) {
        frameAt = now;
        blackSince = -1;
    }

    void frame(long now) { frameAt = now; }

    void sample(long now, boolean black) {
        if (!black) blackSince = -1;
        else if (blackSince < 0) blackSince = now;
    }

    String warning(long now, boolean observing) {
        if (!observing) { reset(now); return null; }
        String problem = blackSince >= 0 && now - blackSince >= BLACK_MS
                ? "화면 캡처가 검은 화면만 받음"
                : now - frameAt >= STALE_MS ? "화면 캡처가 멈춤 (새 화면이 안 들어옴)" : null;
        if (problem == null || warnedAt >= 0 && now - warnedAt < WARN_GAP_MS) return null;
        warnedAt = now;
        return problem;
    }

    static boolean isBlack(ByteBuffer bytes, int width, int height, int rowStride, int pixelStride) {
        if (width < 1 || height < 1 || rowStride < 1 || pixelStride < 3) return false;
        long rowBytes = (width - 1L) * pixelStride + 3;
        long length = (height - 1L) * rowStride + rowBytes;
        if (rowStride < rowBytes || length > bytes.limit()) return false;
        int seen = 0, lit = 0;
        for (int y = height / 40; y < height; y += Math.max(1, height / 20)) {
            for (int x = width / 60; x < width; x += Math.max(1, width / 30)) {
                int at = (int) ((long) y * rowStride + (long) x * pixelStride);
                seen++;
                if ((bytes.get(at) & 255) + (bytes.get(at + 1) & 255) + (bytes.get(at + 2) & 255) > 36) lit++;
            }
        }
        return seen > 100 && lit < seen * 0.01;
    }
}
