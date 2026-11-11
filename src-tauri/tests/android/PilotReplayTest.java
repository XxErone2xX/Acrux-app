package com.acrux.next;

import java.util.List;

/** Pure Java regression checks for the accessibility timeline, without a device. */
public final class PilotReplayTest {
    static final class Hand implements Pilot.Hand {
        long time = 1_000_000_000L;
        long userDown;
        boolean interrupt;
        boolean held;
        int presses;

        public boolean ready(boolean stick) { return true; }
        public boolean fresh(double[] dir, double frac, boolean jump, long ms, boolean keep) {
            time += (ms + 6) * Pilot.MS;
            presses++;
            held = keep;
            return true;
        }
        public boolean cont(double[] dir, long ms, boolean liftStick, boolean liftJump) {
            if (interrupt) {
                interrupt = false;
                time += (ms / 2 + 6) * Pilot.MS;
                held = false;
                return false;
            }
            time += (ms + 6) * Pilot.MS;
            held = !liftStick;
            return true;
        }
        public void forget() { held = false; }
        public long nanos() { return time; }
        public void sleepUntil(long until) { time = Math.max(time, until); }
        public long userDown() { return userDown; }
    }

    public static void main(String[] args) {
        List<TraceEvent> route = List.of(
            new TraceEvent('F', 0, 0, -1, true, false),
            new TraceEvent('M', 600, -1, -1, false, false),
            new TraceEvent('U', 1500, 0, 0, true, false)
        );
        Hand normal = new Hand();
        new Pilot(normal).play(route, () -> {});

        Hand interrupted = new Hand();
        interrupted.interrupt = true;
        Pilot replay = new Pilot(interrupted);
        replay.play(route, () -> {});
        require(replay.cuts == 1, "An interrupted touch must be pressed again");
        require(interrupted.presses == normal.presses + 1, "One recovery press is required");
        require(replay.lostMs >= 30, "Recovery must compensate for lost movement time");
        require(interrupted.time >= normal.time + 30 * Pilot.MS, "The rest of the route must be delayed");
        require(!interrupted.held, "The final finger must be released");

        Hand recording = new Hand();
        Pilot recorder = new Pilot(recording);
        recorder.set(new double[]{0, -1});
        recorder.step();
        recording.userDown = recording.time;
        recorder.step();
        require(recording.held, "A user touch must not discard a gesture that still works");
        recorder.set(null);
        recorder.step();
        require(!recording.held, "Stopping must release a surviving gesture");
        System.out.println("Pilot replay and recording regression checks passed");
    }

    static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
