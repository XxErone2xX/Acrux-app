package com.acrux.next

import kotlin.math.abs

/** Uses the actual picker conversion functions without an Android service or native library. */
object PositionPickerCoordinatesTest {
    @JvmStatic
    fun main(args: Array<String>) {
        check(PickerCoordinates.local(.5, 1920, 80, 1760) == 880f) {
            "Saved physical coordinates must subtract the measured view origin"
        }
        check(PickerCoordinates.normalized(880f, 80, 1920) == .5) {
            "Confirmation must invert the same local-to-physical transform"
        }
        check(PickerCoordinates.local(null, 1080, 26, 1000) == 514f) {
            "An unset pointer starts at the physical display center, not the inset view center"
        }
        for ((physical, origin, extent) in listOf(
            Triple(2400, 72, 2280), Triple(1080, 26, 1010),
            Triple(1080, 48, 1000), Triple(2400, 0, 2400),
        )) {
            for (saved in listOf(.2, .5, .75)) {
                var value = saved
                repeat(100) {
                    val local = PickerCoordinates.local(value, physical, origin, extent)
                    value = PickerCoordinates.normalized(local, origin, physical)
                }
                check(abs(value - saved) * physical < .001) {
                    "Repeated reopen/confirm must not accumulate the window inset"
                }
            }
        }
        // Current layout geometry is used independently for portrait/landscape.
        for (physical in listOf(1080, 2400)) {
            val local = PickerCoordinates.local(.4, physical, 48, physical - 96)
            check(abs(PickerCoordinates.normalized(local, 48, physical) - .4) < .000001) {
                "Rotation cannot divide current coordinates by a stale display size"
            }
        }
        val clipped = PickerCoordinates.local(.99, 1000, 40, 800)
        check(clipped == 799f && PickerCoordinates.normalized(clipped, 40, 1000) == .839) {
            "A point outside the available view saves the actual displayed, clamped position"
        }
        check(PickerCoordinates.clamp(-50f, 800) == 1f
                && PickerCoordinates.clamp(900f, 800) == 799f
                && PickerCoordinates.clamp(30f, 1) == 0f) {
            "Dragging and layout clamping share the same finite view bounds"
        }
        println("Position picker origin and round-trip regression checks passed")
    }
}
