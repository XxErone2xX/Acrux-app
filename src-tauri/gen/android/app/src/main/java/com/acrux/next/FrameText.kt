package com.acrux.next

import android.graphics.Bitmap
import android.media.Image
import com.google.android.gms.tasks.Tasks
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import com.google.mlkit.vision.text.korean.KoreanTextRecognizerOptions
import com.google.mlkit.vision.text.japanese.JapaneseTextRecognizerOptions
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.TimeUnit

/** The stable app's offline OCR, requested only for text-dependent states. */
object FrameText {
    private val recognizer by lazy {
        when (java.util.Locale.getDefault().language) {
            "ko" -> TextRecognition.getClient(KoreanTextRecognizerOptions.Builder().build())
            "ja" -> TextRecognition.getClient(JapaneseTextRecognizerOptions.Builder().build())
            else -> TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
        }
    }

    fun read(image: Image, x: Int, y: Int, width: Int, height: Int): String {
        val plane = image.planes[0]
        val buffer = plane.buffer.duplicate()
        val pixels = IntArray(width * height)
        for (row in 0 until height) for (col in 0 until width) {
            val at = (y + row) * plane.rowStride + (x + col) * plane.pixelStride
            pixels[row * width + col] = (0xff shl 24) or
                ((buffer.get(at).toInt() and 255) shl 16) or
                ((buffer.get(at + 1).toInt() and 255) shl 8) or (buffer.get(at + 2).toInt() and 255)
        }
        val bitmap = Bitmap.createBitmap(pixels, width, height, Bitmap.Config.ARGB_8888)
        try {
            val result = Tasks.await(recognizer.process(InputImage.fromBitmap(bitmap, 0)), 3, TimeUnit.SECONDS)
            val words = JSONArray()
            for (block in result.textBlocks) for (line in block.lines) {
                val rect = line.boundingBox ?: continue
                if (words.length() >= 1024) break
                words.put(JSONObject().put("text", line.text.take(512)).put("rect", JSONArray(listOf(
                    ((x + rect.left).toDouble() / image.width).coerceIn(0.0, 1.0),
                    ((y + rect.top).toDouble() / image.height).coerceIn(0.0, 1.0),
                    ((x + rect.right).toDouble() / image.width).coerceIn(0.0, 1.0),
                    ((y + rect.bottom).toDouble() / image.height).coerceIn(0.0, 1.0),
                ))))
            }
            return words.toString()
        } catch (error: Exception) {
            if (error is InterruptedException) Thread.currentThread().interrupt()
            throw IllegalStateException("화면의 글자를 읽지 못했습니다: ${error.javaClass.simpleName}", error)
        } finally { bitmap.recycle() }
    }
}
