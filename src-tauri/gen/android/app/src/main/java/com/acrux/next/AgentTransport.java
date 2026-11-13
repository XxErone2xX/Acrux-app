package com.acrux.next;

import java.nio.ByteBuffer;

/** Bounds and single-input ownership shared by the Android agent transport. */
final class AgentTransport {
    static final int MAX_COPY_BYTES = 64 * 1024 * 1024;
    static final int MAX_FRAME_BYTES = 256 * 1024 * 1024;
    static final long FRAME_TTL_MS = 250;

    static boolean region(int width, int height, int x, int y, int w, int h) {
        return width > 0 && height > 0 && width <= 16384 && height <= 16384
                && (long) width * height * 4 <= MAX_FRAME_BYTES
                && x >= 0 && y >= 0 && w > 0 && h > 0
                && (long) x + w <= width && (long) y + h <= height;
    }

    static boolean fresh(long receivedAt, long now, int frameGeneration, int requestGeneration) {
        return frameGeneration == requestGeneration && now >= receivedAt && now - receivedAt < FRAME_TTL_MS;
    }
    static long captureAt(long nowMs,long nowNanos,long imageNanos){
        if(imageNanos<=0 || nowNanos<imageNanos)return -1;
        long age=(nowNanos-imageNanos)/1_000_000;
        return age<FRAME_TTL_MS?nowMs-age:-1;
    }

    static int copySize(int width, int height, int x, int y, int w, int h) {
        long size = (long) w * h * 4;
        if (!region(width, height, x, y, w, h) || size > MAX_COPY_BYTES) {
            throw new IllegalArgumentException("화면 관측 영역이 올바르지 않습니다.");
        }
        return (int) size;
    }

    static void copy(ByteBuffer source, int width, int height, int row, int pixel,
                     int x, int y, int w, int h, ByteBuffer target) {
        int size = copySize(width, height, x, y, w, h);
        long rowBytes = (width - 1L) * pixel + 3;
        long length = (height - 1L) * row + rowBytes;
        if (pixel < 3 || row < rowBytes || length > source.limit() || target.capacity() < size) {
            throw new IllegalArgumentException("화면 관측 데이터가 올바르지 않습니다.");
        }
        target.clear();
        target.limit(size);
        for (int yy = y; yy < y + h; yy++) {
            int start = (int) ((long) yy * row + (long) x * pixel);
            // The final alpha byte need not exist in an RGB-only plane.
            if (pixel == 4 && (long) start + w * 4L <= source.limit()) {
                ByteBuffer line = source.duplicate();
                line.position(start); line.limit(start + w * 4);
                target.put(line);
            } else {
                for (int xx = 0; xx < w; xx++) {
                    int at = start + xx * pixel;
                    target.put(source.get(at)).put(source.get(at + 1)).put(source.get(at + 2)).put((byte) 255);
                }
            }
        }
        target.flip();
    }

    static boolean tap(int[] packet) {
        return gesture(packet) && packet[4] == 0;
    }

    // [2,id,W,H,kind,duration,x0,y0,generation,...]. Generation always stays at 8.
    // Tap: 9 values. Drag: 11 (+end). Pinch: 15 (+end1,start2,end2).
    // Stick: 13 (+end, optional jump point; -1,-1 means no jump).
    static boolean gesture(int[] packet) {
        if (packet == null || packet.length < 9 || packet[0] != 2
                || !region(packet[2], packet[3], 0, 0, packet[2], packet[3])
                || packet[5] < 1 || packet[5] > 1000 || !pointInside(packet, 6)) return false;
        switch (packet[4]) {
            case 0: return packet.length == 9;
            case 1: return packet.length == 11 && pointInside(packet, 9);
            case 2: return packet.length == 15 && pointInside(packet, 9)
                    && pointInside(packet, 11) && pointInside(packet, 13);
            case 3: return packet.length == 13 && pointInside(packet, 9)
                    && ((packet[11] == -1 && packet[12] == -1) || pointInside(packet, 11));
            default: return false;
        }
    }

    private static boolean pointInside(int[] packet, int at) {
        return packet[at] >= 0 && packet[at] < packet[2]
                && packet[at + 1] >= 0 && packet[at + 1] < packet[3];
    }

    static boolean viewport(int[] packet, int width, int height) {
        return command(packet) && packet[2] == width && packet[3] == height;
    }

    static boolean command(int[] packet) {
        if (gesture(packet)) return true;
        if (packet == null || packet.length < 9 || packet[0] != 2
                || packet[5] < 1 || packet[5] > 5000 || packet[7] != 0
                || !region(packet[2], packet[3], 0, 0, packet[2], packet[3])) return false;
        if (packet[4] == 5) return packet.length == 9 && packet[6] >= 0 && packet[6] <= 255;
        if ((packet[4] != 4 && packet[4] != 6) || packet.length > 2057 || packet[6] < 0 || packet[6] > 1) return false;
        for (int at = 9; at < packet.length; at++) if (packet[at] < 1 || packet[at] > 65535) return false;
        return true;
    }

    static String text(int[] packet) {
        StringBuilder text = new StringBuilder(packet.length - 9);
        for (int at = 9; at < packet.length; at++) text.append((char) packet[at]);
        return text.toString();
    }

    static int fingers(int[] packet) {
        return packet[4] == 2 || packet[4] == 3 && packet[11] >= 0 ? 2 : 1;
    }

    static int fingerDuration(int[] packet, int finger) {
        // Jump is one simultaneous press, never repeated while the stick stays down.
        return packet[4] == 3 && finger == 1 ? Math.min(packet[5], 50) : packet[5];
    }

    static float[] position(int[] packet, int finger, int elapsed) {
        int start = finger == 0 ? 6 : 11;
        int end = finger == 0 ? 9 : 13;
        if (packet[4] == 0 || packet[4] == 3 && finger == 1) end = start;
        int moveMs = packet[4] == 3 ? Math.min(packet[5], 50) : packet[5];
        double fraction = Math.max(0, Math.min(moveMs, elapsed)) / (double) moveMs;
        float[] point = new float[]{
                (float) (packet[start] + (packet[end] - packet[start]) * fraction),
                (float) (packet[start + 1] + (packet[end + 1] - packet[start + 1]) * fraction)
        };
        if (packet[4] == 3 && finger == 0 && packet[5] > 50 && elapsed > 50) {
            // Like Joy.H.cont: stationary continuations can be canceled by
            // Android/game input. Oscillate at most one pixel without drift.
            int held = Math.min(elapsed, packet[5]) - 50;
            int interval = held / 50;
            int duration = Math.max(1, Math.min(50, packet[5] - 50 - interval * 50));
            double progress = Math.min(1, (held % 50) / (double) duration);
            double offset = interval % 2 == 0 ? progress : 1 - progress;
            int axis = packet[2] > 1 ? 0 : 1;
            int extent = axis == 0 ? packet[2] : packet[3];
            if (extent > 1) point[axis] += (point[axis] < extent - 1 ? 1 : -1) * offset;
        }
        return point;
    }

    static float[] releasePosition(int[] packet, float[] from) {
        float[] to = from.clone();
        int axis = packet[2] > 1 ? 0 : 1;
        int extent = axis == 0 ? packet[2] : packet[3];
        if (extent > 1) to[axis] += to[axis] <= extent - 2 ? 1 : -1;
        return to;
    }

    static boolean intersects(float x0, float y0, float x1, float y1,
                              int left, int top, int right, int bottom) {
        if (!Float.isFinite(x0) || !Float.isFinite(y0) || !Float.isFinite(x1) || !Float.isFinite(y1)
                || right < left || bottom < top) return false;
        double enter = 0, leave = 1;
        double[] from = {x0, y0}, to = {x1, y1}, low = {left, top}, high = {right, bottom};
        for (int axis = 0; axis < 2; axis++) {
            double delta = to[axis] - from[axis];
            if (delta == 0) {
                if (from[axis] < low[axis] || from[axis] > high[axis]) return false;
            } else {
                double a = (low[axis] - from[axis]) / delta, b = (high[axis] - from[axis]) / delta;
                enter = Math.max(enter, Math.min(a, b));
                leave = Math.min(leave, Math.max(a, b));
                if (enter > leave) return false;
            }
        }
        return true;
    }

    static final class Input {
        private boolean busy;
        private int generation, id;
        synchronized boolean busy() { return busy; }
        synchronized boolean begin(int nextGeneration, int nextId) {
            if (busy) return false;
            busy = true; generation = nextGeneration; id = nextId;
            return true;
        }
        synchronized boolean current(int expectedGeneration, int expectedId) {
            return busy && generation == expectedGeneration && id == expectedId;
        }
        synchronized boolean finish(int expectedGeneration, int expectedId) {
            if (!current(expectedGeneration, expectedId)) return false;
            busy = false;
            return true;
        }
    }
}
