package com.acrux.next;

import java.nio.ByteBuffer;

/** Pure Java tests for capture warning thresholds, with no live screen or device. */
public final class CaptureHealthTest {
    public static void main(String[] args) {
        CaptureHealth health = new CaptureHealth();
        health.reset(0);
        require(health.warning(7999, true) == null, "A delayed first frame gets an eight-second grace period");
        require(health.warning(8000, true).contains("멈춤"), "No callbacks must warn at eight seconds");
        require(health.warning(37999, true) == null, "Warnings must not repeat within thirty seconds");
        require(health.warning(38000, true) != null, "Persistent capture loss must warn again");

        health = new CaptureHealth();
        health.reset(100);
        health.sample(100, true);
        health.frame(6099);
        require(health.warning(6099, true) == null, "Short black loading screens are ignored");
        health.frame(6100);
        require(health.warning(6100, true).contains("검은"), "Continuous black frames warn at six seconds");
        health.sample(6200, false);
        health.frame(36100);
        require(health.warning(36100, true) == null, "Recovered pixels clear black evidence");

        health = new CaptureHealth();
        health.reset(0);
        health.sample(0, true);
        require(health.warning(100000, false) == null, "Idle or locked sessions never warn");
        require(health.warning(107999, true) == null, "Resuming begins a fresh delivery grace period");
        health.reset(108000);
        require(health.warning(115999, true) == null, "Resizing resets the delivery grace period");

        ByteBuffer pixels = ByteBuffer.allocate(256 * 40);
        for (int y = 0; y < 40; y++) {
            for (int x = 0; x < 60; x++) pixels.put(y * 256 + x * 4 + 3, (byte) 255);
            for (int padding = 240; padding < 256; padding++) pixels.put(y * 256 + padding, (byte) 255);
        }
        require(CaptureHealth.isBlack(pixels, 60, 40, 256, 4), "Alpha and row padding are not lit pixels");
        for (int i = 0; i < 6; i++) pixels.put(256 + (1 + i * 2) * 4, (byte) 255);
        require(!CaptureHealth.isBlack(pixels, 60, 40, 256, 4), "Visible content at one percent is not a black capture");
        require(!CaptureHealth.isBlack(ByteBuffer.allocate(8), 60, 40, 256, 4), "Truncated pixels are unknown, not black");
        require(!CaptureHealth.isBlack(pixels, 60, 40, 20, 4), "Overlapping rows cannot establish black evidence");
        System.out.println("Capture health regression checks passed");
    }

    static void require(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }
}
