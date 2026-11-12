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
        return packet != null && packet.length == 9 && packet[0] == 2 && packet[4] == 0
                && packet[2] > 0 && packet[3] > 0 && packet[5] >= 1 && packet[5] <= 1000
                && packet[6] >= 0 && packet[6] < packet[2] && packet[7] >= 0 && packet[7] < packet[3];
    }

    static boolean viewport(int[] packet, int width, int height) {
        return tap(packet) && packet[2] == width && packet[3] == height;
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
