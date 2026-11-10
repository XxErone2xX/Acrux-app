package com.acrux.next;

/** Actual Android finger event, with time relative to the recording start in milliseconds. */
public final class TraceEvent {
    public char k;
    public double t, dx, dy;
    public boolean a, b;
    public TraceEvent(char k, double t, double dx, double dy, boolean a, boolean b) {
        this.k = k;
        this.t = t;
        this.dx = dx;
        this.dy = dy;
        this.a = a;
        this.b = b;
    }
}
