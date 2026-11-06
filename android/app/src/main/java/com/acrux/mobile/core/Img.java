package com.acrux.mobile.core;

/** RGB 이미지 (행 우선 · 0~255) — PC 버전의 numpy (h, w, 3) 배열과 같은 역할 */
public final class Img {
    public final int w, h;
    public final int[] r, g, b;

    public Img(int w, int h) {
        this.w = w;
        this.h = h;
        int n = Math.max(0, w * h);
        r = new int[n];
        g = new int[n];
        b = new int[n];
    }

    /** RGBA 바이트 (한 줄 = rowStride 바이트 · 한 칸 = pixelStride 바이트) 의 (x, y, w, h) 부분 */
    public static Img fromRgba(byte[] src, int rowStride, int pixelStride, int x0, int y0, int w, int h) {
        Img im = new Img(w, h);
        for (int y = 0; y < h; y++) {
            int o = (y0 + y) * rowStride + x0 * pixelStride, k = y * w;
            for (int x = 0; x < w; x++, o += pixelStride, k++) {
                im.r[k] = src[o] & 255;
                im.g[k] = src[o + 1] & 255;
                im.b[k] = src[o + 2] & 255;
            }
        }
        return im;
    }

    public int i(int x, int y) {
        return y * w + x;
    }

    public int max(int k) {
        return Math.max(r[k], Math.max(g[k], b[k]));
    }

    public int min(int k) {
        return Math.min(r[k], Math.min(g[k], b[k]));
    }

    /** 세로 [y0, y1) · 가로 [x0, x1) 부분 (범위 밖은 잘라냄) */
    public Img crop(int x0, int y0, int x1, int y1) {
        x0 = Math.max(0, x0);
        y0 = Math.max(0, y0);
        x1 = Math.min(w, x1);
        y1 = Math.min(h, y1);
        Img o = new Img(Math.max(0, x1 - x0), Math.max(0, y1 - y0));
        for (int y = 0; y < o.h; y++) {
            for (int x = 0; x < o.w; x++) {
                int s = (y0 + y) * w + x0 + x, d = y * o.w + x;
                o.r[d] = r[s];
                o.g[d] = g[s];
                o.b[d] = b[s];
            }
        }
        return o;
    }
}
