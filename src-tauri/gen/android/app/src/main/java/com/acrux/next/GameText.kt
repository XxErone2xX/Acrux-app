package com.acrux.next

import android.os.Build
import android.view.KeyEvent
import android.view.inputmethod.EditorInfo

/** Uses the game's editor without replacing the user's keyboard on Android 13+. */
object GameText {
    fun commit(service: TapService,text:String,submit:Boolean):Boolean {
        if(!service.canInput()||text.length>1024||text.any{it=='\u0000'})return false
        return if(Build.VERSION.SDK_INT>=33) Api33.commit(service,text,submit) else MacroInput.commit(text,submit)
    }
    fun submit(service:TapService):Boolean {
        if(!service.canInput())return false
        return if(Build.VERSION.SDK_INT>=33)Api33.submit(service) else MacroInput.submit()
    }
    @android.annotation.TargetApi(33)
    private object Api33 {
        fun commit(service:TapService,text:String,submit:Boolean):Boolean{
            val method=service.inputMethod ?: return false
            if(method.currentInputEditorInfo?.packageName!="com.roblox.client")return false
            val connection=method.currentInputConnection ?: return false
            connection.performContextMenuAction(android.R.id.selectAll)
            connection.deleteSurroundingText(4096,4096)
            connection.commitText(text,1,null)
            return !submit||Api33.submit(service)
        }
        fun submit(service:TapService):Boolean{
            val method=service.inputMethod ?: return false
            val info=method.currentInputEditorInfo ?: return false
            if(info.packageName!="com.roblox.client")return false
            val connection=method.currentInputConnection ?: return false
            val action=info.imeOptions and EditorInfo.IME_MASK_ACTION
            if(action!=EditorInfo.IME_ACTION_NONE&&action!=EditorInfo.IME_ACTION_UNSPECIFIED)connection.performEditorAction(action)
            else{
                connection.sendKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN,KeyEvent.KEYCODE_ENTER))
                connection.sendKeyEvent(KeyEvent(KeyEvent.ACTION_UP,KeyEvent.KEYCODE_ENTER))
            }
            return true
        }
    }
}
