package com.acrux.next

import android.inputmethodservice.InputMethodService
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.Button
import android.widget.LinearLayout

/** Search/quantity input uses the game's actual input connection, never a clipboard. */
class MacroInput : InputMethodService() {
    override fun onCreate() { super.onCreate(); current = this }
    override fun onDestroy() { if (current === this) current = null; super.onDestroy() }

    private fun gameConnection() = currentInputConnection?.takeIf {
        currentInputEditorInfo?.packageName == "com.roblox.client"
            && TapService.current?.canInput() == true
    }

    private fun write(text: String, submit: Boolean): Boolean {
        val connection = gameConnection() ?: return false
        if (text.length > 1024 || text.any { it == '\u0000' }) return false
        connection.beginBatchEdit()
        val written = try {
            connection.performContextMenuAction(android.R.id.selectAll)
            connection.deleteSurroundingText(4096, 4096)
            connection.commitText(text, 1)
        } finally { connection.endBatchEdit() }
        return written && (!submit || enter())
    }

    private fun enter(): Boolean {
        val connection = gameConnection() ?: return false
        val action = (currentInputEditorInfo?.imeOptions ?: 0) and EditorInfo.IME_MASK_ACTION
        if (action != EditorInfo.IME_ACTION_NONE && action != EditorInfo.IME_ACTION_UNSPECIFIED
            && connection.performEditorAction(action)) return true
        return connection.sendKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_ENTER))
            && connection.sendKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_ENTER))
    }

    override fun onCreateInputView(): View {
        val keyboard = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(0xff202125.toInt())
        }
        for (letters in listOf("1234567890", "qwertyuiop", "asdfghjkl", "zxcvbnm")) {
            val row = LinearLayout(this)
            for (letter in letters) row.addView(Button(this).apply {
                text = letter.toString()
                minWidth = 0; minimumWidth = 0
                setPadding(0, 0, 0, 0)
                setOnClickListener { currentInputConnection?.commitText(letter.toString(), 1) }
            }, LinearLayout.LayoutParams(0, -2, 1f))
            keyboard.addView(row)
        }
        val row = LinearLayout(this)
        fun button(label: String, action: () -> Unit) = row.addView(Button(this).apply {
            text = label; setOnClickListener { action() }
        }, LinearLayout.LayoutParams(0, -2, 1f))
        button("다른 키보드") { getSystemService(InputMethodManager::class.java).showInputMethodPicker() }
        button("Space") { currentInputConnection?.commitText(" ", 1) }
        button("⌫") { currentInputConnection?.deleteSurroundingText(1, 0) }
        button("Enter") { currentInputConnection?.performEditorAction(EditorInfo.IME_ACTION_DONE) }
        keyboard.addView(row)
        return keyboard
    }

    companion object {
        @Volatile private var current: MacroInput? = null
        fun ready() = current != null
        fun commit(text: String, submit: Boolean) = current?.write(text, submit) == true
        fun submit() = current?.enter() == true
    }
}
