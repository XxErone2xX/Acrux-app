package com.acrux.mobile;

import android.graphics.Bitmap;
import android.graphics.Rect;

import com.acrux.mobile.core.Img;
import com.acrux.mobile.core.Mover;
import com.acrux.mobile.core.SellText;
import com.google.android.gms.tasks.Tasks;
import com.google.mlkit.vision.common.InputImage;
import com.google.mlkit.vision.text.Text;
import com.google.mlkit.vision.text.TextRecognition;
import com.google.mlkit.vision.text.TextRecognizer;
import com.google.mlkit.vision.text.latin.TextRecognizerOptions;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;

/** 글자 읽기 (구글 ML Kit · 폰 안에서만 · 인터넷 안 씀) — 판매 보정 · 판매 · 메뉴 리셋에서 버튼 글자를 찾을 때 */
final class MlOcr implements Mover.Ocr {
    static TextRecognizer rec;

    static synchronized TextRecognizer recognizer() {
        if (rec == null) rec = TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS);
        return rec;
    }

    @Override
    public List<SellText.Box> read(double[] region) {
        List<SellText.Box> out = new ArrayList<>();
        CaptureService cap = CaptureService.active;
        if (cap == null) return out;
        int W = cap.width(), H = cap.height();
        double[] r = region != null ? region : new double[]{0, 0, 1, 1};
        int x0 = (int) (Math.min(r[0], r[2]) * W), y0 = (int) (Math.min(r[1], r[3]) * H);
        int x1 = (int) (Math.max(r[0], r[2]) * W), y1 = (int) (Math.max(r[1], r[3]) * H);
        Img im = cap.grab(x0, y0, Math.max(1, x1 - x0), Math.max(1, y1 - y0));
        if (im == null || im.w < 8 || im.h < 8) return out;
        int[] px = new int[im.w * im.h];
        for (int k = 0; k < px.length; k++) px[k] = 0xFF000000 | (im.r[k] << 16) | (im.g[k] << 8) | im.b[k];
        Bitmap bmp = Bitmap.createBitmap(px, im.w, im.h, Bitmap.Config.ARGB_8888);
        try {
            Text t = Tasks.await(recognizer().process(InputImage.fromBitmap(bmp, 0)), 6, TimeUnit.SECONDS);
            for (Text.TextBlock block : t.getTextBlocks()) {
                for (Text.Line line : block.getLines()) {
                    Rect b = line.getBoundingBox();
                    if (b == null) continue;
                    out.add(new SellText.Box(line.getText(), (x0 + b.exactCenterX()) / W, (y0 + b.exactCenterY()) / H,
                            (double) b.width() / W, (double) b.height() / H));
                }
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();              // 멈춤 신호는 그대로 (다음 확인에서 멈춤)
        } catch (Exception e) {
            Hub.log("글자 읽기 실패: " + e.getMessage(), 'd');
        } finally {
            bmp.recycle();
        }
        return out;
    }
}
