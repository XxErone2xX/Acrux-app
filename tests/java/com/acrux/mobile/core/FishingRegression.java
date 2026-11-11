package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.List;

/** Exercises the production Fisher with synthetic screen pixels and recorded taps. */
public final class FishingRegression {
    static final class Finished extends RuntimeException {}

    static final class Screen implements Fisher.Screen, Fisher.Input {
        final String mode;
        final Img fish = image("fish"), exit = image("exit"), unknown = image("");
        final List<Double> taps = new ArrayList<>();
        int lateReads;

        Screen(String mode) { this.mode = mode; }
        public int width() { return 600; }
        public int height() { return 400; }

        public Img grab(int x, int y, int w, int h) {
            boolean full = x == 0 && y == 0 && w == 600 && h == 400;
            Img current = fish;
            if (mode.equals("exit")) current = exit;
            else if (mode.equals("unknown") && !full) current = unknown;
            else if (!taps.isEmpty()) {
                double elapsed = Fisher.now() - taps.get(0);
                if (mode.equals("slow") && elapsed >= 2.5) current = exit;
                if (mode.equals("race") && elapsed > Fisher.CAST_WAIT && w < 50) {
                    lateReads++;
                    if (lateReads > 1) current = exit;
                }
            }
            return current.crop(x, y, x + w, y + h);
        }

        public void tap(int x, int y, int ms) { taps.add(Fisher.now()); }
    }

    static Img image(String button) {
        Img im = new Img(600, 400);
        for (int dx = 0; dx < 12; dx++) for (int dy = 0; dy < 2; dy++) {
            white(im, 150 + dx, 200 + dy);
            white(im, 450 - dx, 200 + dy);
            white(im, 150 + dx, 300 - dy);
            white(im, 450 - dx, 300 - dy);
            white(im, 150 + dy, 200 + dx);
            white(im, 450 - dy, 200 + dx);
            white(im, 150 + dy, 300 - dx);
            white(im, 450 - dy, 300 - dx);
        }
        if (!button.isEmpty()) for (int y = 280; y < 285; y++) for (int x = 222; x < 227; x++) {
            int i = im.i(x, y);
            im.r[i] = button.equals("exit") ? 240 : 20;
            im.g[i] = button.equals("exit") ? 30 : 100;
            im.b[i] = button.equals("exit") ? 30 : 240;
        }
        return im;
    }

    static void white(Img im, int x, int y) {
        int i = im.i(x, y);
        im.r[i] = im.g[i] = im.b[i] = 255;
    }

    static Fisher make(Screen screen, int retries) {
        Settings cfg = new Settings();
        cfg.apply("panel_region", new double[]{0.25, 0.5, 0.75, 0.75});
        cfg.deriveReel();
        cfg.castRetry = retries;
        return new Fisher(screen, screen, new Fisher.Out() {
            public void log(String text, char color) {}
            public void state(String text) {
                if (text.equals("입질 기다리는 중") || text.equals("자동 보정: 입질 기다리는 중")) throw new Finished();
            }
        }, cfg);
    }

    static void expect(boolean result, String message) {
        if (!result) throw new AssertionError(message);
    }

    static void loop(String mode, int retries, int taps, int full) {
        Screen screen = new Screen(mode);
        Fisher fisher = make(screen, retries);
        try { fisher.loop(); }
        catch (Finished | Fisher.Stopped done) {}
        expect(screen.taps.size() == taps, mode + " loop tap count " + screen.taps.size());
        expect(fisher.full == full, mode + " loop falsely reported inventory full");
        if (full > 0) for (int i = 1; i < screen.taps.size(); i++) {
            expect(screen.taps.get(i) - screen.taps.get(i - 1) >= Fisher.CAST_WAIT, "cast retried too soon");
        }
    }

    static void calibrate(String mode, int taps) {
        Screen screen = new Screen(mode);
        Fisher fisher = make(screen, 3);
        expect(fisher.frameOf(screen.fish, 0) != null, "invalid test fishing frame");
        try { fisher.calibrate(); }
        catch (Finished done) {}
        expect(screen.taps.size() == taps, mode + " calibration tap count " + screen.taps.size());
    }

    public static void main(String[] args) {
        loop("slow", 3, 1, 0);
        loop("exit", 3, 0, 0);
        loop("race", 1, 1, 0);
        loop("full", 3, 3, 1);
        calibrate("slow", 1);
        calibrate("exit", 0);
        calibrate("unknown", 0);
        System.out.println("7 mobile fishing regression cases passed");
    }
}
