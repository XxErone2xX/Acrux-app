package com.acrux.next

import android.graphics.Bitmap
import com.google.android.gms.tasks.Tasks
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import org.json.JSONArray
import org.json.JSONObject
import java.nio.ByteBuffer
import java.util.concurrent.TimeUnit
import kotlin.math.ceil
import kotlin.math.floor

/** Only the platform OCR runs here. Coordinates passed to Rust are full-frame fractions. */
object ScreenOcr {
    private val recognizer by lazy { TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS) }

    @JvmStatic
    fun read(
        frame: ByteBuffer, width: Int, height: Int, row: Int, pixel: Int,
        left: Double, top: Double, right: Double, bottom: Double,
    ): String {
        val x = floor(left * width).toInt().coerceIn(0, width - 1)
        val y = floor(top * height).toInt().coerceIn(0, height - 1)
        val w = ceil(right * width).toInt().coerceIn(x + 1, width) - x
        val h = ceil(bottom * height).toInt().coerceIn(y + 1, height) - y
        if (pixel < 3 || w.toLong() * h > 8_388_608L) return "null"
        val pixels = IntArray(w * h)
        for (yy in 0 until h) {
            for (xx in 0 until w) {
                val at = (y + yy) * row + (x + xx) * pixel
                pixels[yy * w + xx] = (0xff shl 24) or ((frame[at].toInt() and 255) shl 16) or
                    ((frame[at + 1].toInt() and 255) shl 8) or (frame[at + 2].toInt() and 255)
            }
        }
        val bitmap = Bitmap.createBitmap(pixels, w, h, Bitmap.Config.ARGB_8888)
        return try {
            val task = recognizer.process(InputImage.fromBitmap(bitmap, 0))
            task.addOnCompleteListener { bitmap.recycle() }
            val result = Tasks.await(task, 6, TimeUnit.SECONDS)
            JSONArray().apply {
                for (block in result.textBlocks) for (line in block.lines) {
                    val box = line.boundingBox ?: continue
                    put(JSONObject().apply {
                        put("text", line.text)
                        put("rect", JSONArray(listOf(
                            (x + box.left).toDouble() / width, (y + box.top).toDouble() / height,
                            (x + box.right).toDouble() / width, (y + box.bottom).toDouble() / height,
                        )))
                    })
                }
            }.toString()
        } catch (_: Exception) { "null" }
    }
}
