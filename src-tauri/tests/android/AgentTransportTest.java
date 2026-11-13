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
        require(AgentTransport.captureAt(1000,1_000_000_000L,800_000_000L)==800,
                "Frame age starts at compositor capture, not delayed receipt");
        require(AgentTransport.captureAt(1000,1_000_000_000L,700_000_000L)==-1
                && AgentTransport.captureAt(1000,1_000_000_000L,1_000_000_001L)==-1,
                "Old or inconsistent capture timestamps cannot start a fresh observation");
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
        gesturePackets();

        AgentTransport.Input gate = new AgentTransport.Input();
        require(gate.begin(7, 10), "The first input owns the device");
        require(!gate.begin(7, 11) && !gate.finish(7, 11) && gate.busy(), "A second input cannot replace an in-flight input");
        require(gate.finish(7, 10) && !gate.finish(7, 10), "An input is acknowledged once");
        require(gate.begin(8, 12), "A later generation may begin after release");
        require(!gate.finish(7, 10) && gate.current(8, 12), "Late completion cannot clear a newer input");
        require(gate.finish(8, 12) && !gate.busy(), "The matching completion releases the device");
        System.out.println("Agent ROI and input ownership regression checks passed");
    }

    private static void gesturePackets() {
        int[] drag = {2, 21, 1920, 1080, 1, 200, 1200, 250, 9, 1600, 650};
        int[] pinch = {2, 22, 1920, 1080, 2, 250, 800, 500, 9, 900, 500, 1200, 500, 1100, 500};
        int[] stick = {2, 23, 1920, 1080, 3, 300, 180, 800, 9, 240, 760, -1, -1};
        int[] jump = {2, 24, 1920, 1080, 3, 300, 180, 800, 9, 240, 760, 1700, 850};
        for (int[] packet : new int[][]{drag, pinch, stick, jump}) {
            require(AgentTransport.gesture(packet) && AgentTransport.viewport(packet, 1920, 1080),
                    "Drag, pinch and stick commands share the viewport guard");
            require(!AgentTransport.tap(packet), "Movement commands never masquerade as fishing taps");
            require(packet[8] == 9 && packet[1] >= 21, "Generation and immutable input id keep their header positions");
            for (int finger = 0; finger < AgentTransport.fingers(packet); finger++) {
                int end = AgentTransport.fingerDuration(packet, finger);
                int elapsed = 0;
                float[] previousEnd = null;
                while (elapsed < end) {
                    int next = Math.min(elapsed + 50, end);
                    float[] from = AgentTransport.position(packet, finger, elapsed);
                    float[] to = AgentTransport.position(packet, finger, next);
                    require(next > elapsed && next - elapsed <= 50 && to[0] >= 0 && to[0] < packet[2]
                                    && to[1] >= 0 && to[1] < packet[3],
                            "Every actual gesture fragment stays bounded and inside the viewport");
                    if (packet[4] == 3 && finger == 0 && elapsed >= 50) {
                        require(from[0] != to[0] || from[1] != to[1],
                                "A held joystick continuation must move instead of submitting a zero-length path");
                    }
                    if (previousEnd != null) require(from[0] == previousEnd[0] && from[1] == previousEnd[1],
                            "Continuation starts at the previous fragment endpoint");
                    previousEnd = to;
                    elapsed = next;
                }
                require(elapsed == end, "Each finger reaches its release boundary without extending the command");
            }
        }
        float[] middle = AgentTransport.position(drag, 0, 100);
        require(middle[0] == 1400 && middle[1] == 450, "Camera drag follows its measured linear path");
        require(AgentTransport.position(pinch, 1, 250)[0] == 1100,
                "Pinch preserves both separately bounded finger paths");
        require(AgentTransport.fingers(stick) == 1 && AgentTransport.fingers(jump) == 2,
                "Optional jump adds a finger only when an actual jump point is provided");
        require(AgentTransport.position(stick, 0, 50)[0] == 240
                        && Math.abs(AgentTransport.position(stick, 0, 300)[0] - 240) <= 1,
                "The joystick reaches its endpoint in the first fragment then stays within one pixel without drift");
        require(AgentTransport.fingerDuration(jump, 1) == 50 && AgentTransport.fingerDuration(jump, 0) == 300,
                "Jump releases once while the same joystick contact can continue");
        int[] invalid = jump.clone(); invalid[12] = -1;
        require(!AgentTransport.gesture(invalid), "Half-missing jump points are rejected");
        invalid = pinch.clone(); invalid[14] = 1080;
        require(!AgentTransport.gesture(invalid), "The second finger cannot end outside the viewport");
        invalid = drag.clone(); invalid[9] = -1;
        require(!AgentTransport.gesture(invalid), "Drag cannot cross the viewport through an invalid endpoint");
        invalid = drag.clone(); invalid[5] = 1001;
        require(!AgentTransport.gesture(invalid), "A movement command cannot exceed one second");
        invalid = drag.clone(); invalid[4] = 2;
        require(!AgentTransport.gesture(invalid), "Truncated pinch packets are rejected before coordinates are read");
        invalid = jump.clone(); invalid[4] = 4;
        require(!AgentTransport.gesture(invalid), "Unknown input kinds are rejected");
        require(AgentTransport.intersects(0, 50, 100, 50, 40, 40, 60, 60),
                "An overlay crossing is blocked even when both endpoints are outside it");
        require(AgentTransport.intersects(100, 100, 0, 0, 40, 40, 60, 60),
                "Reverse diagonal paths receive the same overlay protection");
        require(AgentTransport.intersects(40, 40, 40, 40, 40, 40, 60, 60),
                "A tap on an overlay edge is conservatively blocked");
        require(!AgentTransport.intersects(0, 39, 100, 39, 40, 40, 60, 60)
                        && !AgentTransport.intersects(0, 0, 30, 30, 40, 40, 60, 60),
                "Paths beyond the rectangle and beyond the segment endpoint remain available");
        heldJoystickBoundaries(stick);
    }

    private static void heldJoystickBoundaries(int[] template) {
        for (int ms : new int[]{51, 75, 99, 100, 101, 149, 150, 200, 999, 1000}) {
            for (int edge : new int[]{0, template[2] - 1}) {
                int[] packet = template.clone(); packet[5] = ms; packet[9] = edge;
                float[] previous = AgentTransport.position(packet, 0, 50);
                for (int elapsed = 50; elapsed < ms;) {
                    int next = Math.min(elapsed + 50, ms);
                    float[] from = AgentTransport.position(packet, 0, elapsed);
                    float[] to = AgentTransport.position(packet, 0, next);
                    require(from[0] == previous[0] && from[1] == previous[1],
                            "Held continuation starts at the actual preceding endpoint");
                    require(from[0] != to[0] || from[1] != to[1],
                            "Even a final fragment shorter than 50 ms must have a nonzero held path");
                    require(to[0] >= 0 && to[0] <= packet[2] - 1 && Math.abs(to[0] - edge) <= 1,
                            "The one-pixel oscillation stays within the physical screen and never accumulates drift");
                    float[] lift = AgentTransport.releasePosition(packet, to);
                    require(lift[0] >= 0 && lift[0] <= packet[2] - 1
                                    && Math.abs(lift[0] - to[0]) == 1,
                            "Cancellation lifts the continued contact with a bounded nonzero path, including screen edges");
                    previous = to; elapsed = next;
                }
            }
        }
        int[] packet = template.clone(); packet[5] = 150;
        float[] start = AgentTransport.position(packet, 0, 0);
        float[] finish = AgentTransport.position(packet, 0, 150);
        float[] from = AgentTransport.position(packet, 0, 50);
        float[] to = AgentTransport.position(packet, 0, 100);
        require(!AgentTransport.intersects(start[0], start[1], finish[0], finish[1], 241, 759, 242, 761)
                        && AgentTransport.intersects(from[0], from[1], to[0], to[1], 241, 759, 242, 761),
                "The actual one-pixel continuation needs its own overlay check even when the overall requested line is clear");
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
