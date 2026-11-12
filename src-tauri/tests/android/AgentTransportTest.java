package com.acrux.next;

import java.nio.ByteBuffer;

/** Real crop copying and input ownership checks, without a device or native library. */
public final class AgentTransportTest {
    public static void main(String[] args) {
        ByteBuffer source = pixels(8, 6, 40, 4);
        source.position(13);
        ByteBuffer crop = ByteBuffer.allocateDirect(3 * 4 * 4);
        AgentTransport.copy(source, 8, 6, 40, 4, 2, 1, 3, 4, crop);
        require(source.position() == 13, "Copying must not change the Image plane's position");
        require(crop.position() == 0 && crop.limit() == 48, "JNI receives only packed ROI pixels");
        for (int y = 0; y < 4; y++) for (int x = 0; x < 3; x++) {
            int at = (y * 3 + x) * 4;
            require((crop.get(at) & 255) == x + 2 && (crop.get(at + 1) & 255) == y + 1,
                    "Crop pixels must retain their original viewport coordinates");
            require((crop.get(at + 3) & 255) == 255, "RGBA alpha is preserved");
        }
        source = pixels(8, 6, 29, 3);
        crop = ByteBuffer.allocateDirect(16);
        AgentTransport.copy(source, 8, 6, 29, 3, 6, 4, 2, 2, crop);
        require((crop.get(12) & 255) == 7 && (crop.get(13) & 255) == 5 && (crop.get(15) & 255) == 255,
                "Padded RGB planes are packed as RGBA without reading beyond the last pixel");
        require(!AgentTransport.region(8, 6, Integer.MAX_VALUE, 0, 4, 2), "Overflowing crop bounds are rejected");
        require(!AgentTransport.region(8, 6, 7, 5, 2, 2), "A crop cannot cross the viewport edge");
        require(!AgentTransport.region(16384, 16384, 0, 0, 1, 1), "Retained full-frame storage is bounded");
        boolean rejected = false;
        try { AgentTransport.copy(ByteBuffer.allocate(8), 8, 6, 40, 4, 0, 0, 2, 2, ByteBuffer.allocateDirect(16)); }
        catch (IllegalArgumentException expected) { rejected = true; }
        require(rejected, "Truncated frames must never be read as black pixels");
        rejected = false;
        try { AgentTransport.copySize(8192, 4096, 0, 0, 8192, 4096); }
        catch (IllegalArgumentException expected) { rejected = true; }
        require(rejected, "A reusable crop buffer cannot grow without a bound");
        require(AgentTransport.fresh(100, 349, 7, 7) && !AgentTransport.fresh(100, 350, 7, 7) && !AgentTransport.fresh(100, 99, 7, 7),
                "Only monotonic frames younger than 250 ms may be observed");
        require(!AgentTransport.fresh(100, 101, 7, 8), "A recent image from before stop/restart cannot serve a new observation");
        // A queued producer image still belongs to its reader's creation
        // generation even when the callback arrives after the next start.
        int readerGeneration = 7, callbackGeneration = 8;
        require(!AgentTransport.fresh(100, 100, readerGeneration, callbackGeneration),
                "Callback delivery cannot relabel an old reader's buffered image");
        readerGeneration = callbackGeneration;
        require(AgentTransport.fresh(101, 101, readerGeneration, callbackGeneration),
                "A replacement producer stream can supply the new generation");

        int[] tap = {2, 10, 1920, 1080, 0, 40, 850, 834, 7};
        require(AgentTransport.viewport(tap, 1920, 1080), "Valid tap envelope matches the viewport");
        require(!AgentTransport.viewport(tap, 1080, 1920), "Rotation invalidates an already planned input");
        tap[6] = 1920;
        require(!AgentTransport.tap(tap), "Input points must remain inside the viewport");
        require(!AgentTransport.tap(new int[]{0, 40, 850, 834, 7}), "Legacy motion packets are not agent input");

        AgentTransport.Input gate = new AgentTransport.Input();
        require(gate.begin(7, 10), "The first input owns the device");
        require(!gate.begin(7, 11) && !gate.finish(7, 11) && gate.busy(), "A second input cannot replace an in-flight input");
        require(gate.finish(7, 10) && !gate.finish(7, 10), "An input is acknowledged once");
        require(gate.begin(8, 12), "A later generation may begin after release");
        require(!gate.finish(7, 10) && gate.current(8, 12), "Late completion cannot clear a newer input");
        require(gate.finish(8, 12) && !gate.busy(), "The matching completion releases the device");
        System.out.println("Agent ROI and input ownership regression checks passed");
    }

    private static ByteBuffer pixels(int w, int h, int stride, int pixel) {
        ByteBuffer bytes = ByteBuffer.allocate(stride * h);
        for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) {
            int at = y * stride + x * pixel;
            bytes.put(at, (byte) x).put(at + 1, (byte) y).put(at + 2, (byte) (x + y));
            if (pixel >= 4) bytes.put(at + 3, (byte) 255);
        }
        return bytes;
    }

    private static void require(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }
}
