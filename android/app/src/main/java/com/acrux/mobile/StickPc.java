package com.acrux.mobile;

import com.acrux.mobile.core.Pc;
import com.acrux.mobile.core.Stick;

import java.util.HashSet;
import java.util.Set;

/**
 * W · A · S · D 는 로블록스 조이스틱을 터치로 밀어서 (Joy) · 나머지 키 · 마우스는 Shizuku 로 (없으면 안 보냄)
 *  W 위 · A 왼쪽 · S 아래 · D 오른쪽 · W+A 왼쪽 위 대각선 …
 */
final class StickPc implements Pc {
    final Pc base;                    // Shizuku (없으면 null)
    final Set<String> held = new HashSet<>();
    final Set<String> warned = new HashSet<>();

    StickPc(Pc base, double[] stick, int sw, int sh) {
        this.base = base;
        Joy.at(stick[0] * sw, stick[1] * sh, stick[2] * sh);
    }

    static boolean wasd(String k) {
        return k.equals("w") || k.equals("a") || k.equals("s") || k.equals("d");
    }

    synchronized void push() {
        Joy.set(Stick.dir(held.contains("w"), held.contains("a"), held.contains("s"), held.contains("d")));
    }

    void noKey(String k) {
        if (base == null && warned.add(k)) Hub.log("'" + k.toUpperCase() + "' 키는 아직 터치로 못 보냄 (Shizuku 필요) — 건너뜀", 'y');
    }

    @Override
    public void keyDown(String k) {
        if (wasd(k)) {
            synchronized (this) {
                if (held.add(k)) push();
            }
        } else if (base != null) base.keyDown(k);
        else noKey(k);
    }

    @Override
    public void keyUp(String k) {
        if (wasd(k)) {
            synchronized (this) {
                if (held.remove(k)) push();
            }
        } else if (base != null) base.keyUp(k);
    }

    @Override
    public void moveTo(int x, int y) {
        if (base != null) base.moveTo(x, y);
    }

    @Override
    public void moveRel(int dx, int dy) {
        if (base != null) base.moveRel(dx, dy);
    }

    @Override
    public void button(String b, boolean down) {
        if (base != null) base.button(b, down);
        else if (down) noKey(b + " click");
    }

    /** 다 뗌 */
    void releaseAll() {
        synchronized (this) {
            held.clear();
        }
        Joy.set(null);
    }
}
